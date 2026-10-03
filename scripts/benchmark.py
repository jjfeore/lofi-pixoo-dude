"""Measure release emitter wall time and quiet bridge CPU/memory on Windows."""
import argparse
import ctypes
from ctypes import wintypes as w
import json
from pathlib import Path
import statistics
import subprocess
import time
import uuid


def stats(process):
    class Memory(ctypes.Structure):
        _fields_ = [("cb", w.DWORD), ("faults", w.DWORD)] + [
            (name, ctypes.c_size_t) for name in ["peak_working", "working", "peak_paged", "paged",
                "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile", "private"]]
    k = ctypes.WinDLL("kernel32", use_last_error=True)
    k.GetProcessTimes.argtypes = [w.HANDLE] + [ctypes.POINTER(w.FILETIME)] * 4
    k.K32GetProcessMemoryInfo.argtypes = [w.HANDLE, ctypes.c_void_p, w.DWORD]
    values = [w.FILETIME() for _ in range(4)]
    assert k.GetProcessTimes(int(process._handle), *(ctypes.byref(x) for x in values))
    cpu = sum((x.dwHighDateTime << 32) | x.dwLowDateTime for x in values[2:]) / 10_000_000
    memory = Memory()
    memory.cb = ctypes.sizeof(memory)
    assert k.K32GetProcessMemoryInfo(int(process._handle), ctypes.byref(memory), memory.cb)
    return {"cpu_seconds": cpu, "working_mib": memory.working / 1048576,
            "private_mib": memory.private / 1048576}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/release/pixoo-pet.exe"))
    parser.add_argument("--output", type=Path, default=Path("local/performance.json"))
    parser.add_argument("--seconds", type=float, default=20)
    parser.add_argument("--pack", type=Path)
    args = parser.parse_args()
    exe = args.exe.resolve()
    root = Path(__file__).resolve().parents[1]
    local = root / "local"
    local.mkdir(exist_ok=True)
    pipe = rf"\\.\pipe\pixoo-benchmark-{uuid.uuid4()}"
    config = local / f"benchmark-{uuid.uuid4()}.toml"
    pack = args.pack.resolve() if args.pack else root / 'pets/diagnostic'
    config.write_text(f"pack = '{pack}'\npipe = '{pipe}'\ndry_run = true\n",
                      encoding="utf-8")
    process = subprocess.Popen([str(exe), "run", "-c", str(config)], stdout=subprocess.DEVNULL,
                               stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        for _ in range(100):
            ready = subprocess.run([str(exe), "status", "--pipe", pipe], capture_output=True,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
            if ready.returncode == 0:
                prepared_bytes = json.loads(ready.stdout)["prepared_bytes"]
                break
            time.sleep(.02)
        else:
            raise RuntimeError("bridge did not start")
        event = json.dumps({"hook_event_name": "SessionStart", "session_id": "benchmark"})
        measures = {}
        for shell in [False, True]:
            times = []
            command = [str(exe), "emit", "--pipe", pipe, "--strict"]
            for index in range(110):
                start = time.perf_counter()
                subprocess.run(command, input=event, text=True, stdout=subprocess.DEVNULL,
                               stderr=subprocess.PIPE, check=True, shell=shell,
                               creationflags=subprocess.CREATE_NO_WINDOW)
                elapsed = (time.perf_counter() - start) * 1000
                if index >= 10:
                    times.append(elapsed)
            measures["cmd_shell" if shell else "direct_process"] = {
                "samples": len(times), "median_ms": statistics.median(times),
                "p95_ms": sorted(times)[94], "max_ms": max(times)}
        before = stats(process)
        started = time.perf_counter()
        time.sleep(args.seconds)
        elapsed = time.perf_counter() - started
        after = stats(process)
        result = {"executable_bytes": exe.stat().st_size, "prepared_bytes": prepared_bytes,
                  "emitter": measures, "quiet_seconds": elapsed,
                  "quiet_cpu_seconds": after["cpu_seconds"] - before["cpu_seconds"],
                  "one_core_cpu_percent": 100 * (after["cpu_seconds"] - before["cpu_seconds"]) / elapsed,
                  "working_mib": after["working_mib"], "private_mib": after["private_mib"],
                  "scope": f"this Windows host; dry-run bridge; pack {pack.name}"}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(json.dumps(result, indent=2))
    finally:
        process.terminate()
        process.wait(timeout=3)
        config.unlink()


if __name__ == "__main__":
    main()
