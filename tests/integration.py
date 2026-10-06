"""Windows CLI/pipe/HTTP tests. Python is a development harness, not a bridge dependency."""
import argparse
import base64
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import sys
import shutil
import threading
import time
import tomllib
import unittest
import uuid
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

EXE = Path(__file__).resolve().parents[1] / "target/release/pixoo-pet.exe"


def cli(*args, data=None, check=True, timeout=5):
    result = subprocess.run([str(EXE), *map(str, args)], input=data, text=True,
                            capture_output=True, check=False, timeout=timeout,
                            creationflags=subprocess.CREATE_NO_WINDOW)
    if check and result.returncode:
        raise AssertionError(f"{args[0]} failed: {result.stderr}")
    return result


class Fixture:
    def __init__(self, root, pack, *, dry=False, fail_once=False, switch_ms=100,
                 storage=False, fetch_gifs=True, start=True, head_only=False,
                 use_default_transport=False):
        self.requests = []
        self.lock = threading.Lock()
        self.fail_once = fail_once
        self.fetch_gifs = fetch_gifs
        self.head_only = head_only
        self.stored_files = {}
        self.cache = root / f"gifs-{uuid.uuid4()}"
        self.process = None
        fixture = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                value = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                with fixture.lock:
                    fixture.requests.append((time.perf_counter(), value))
                    fail = fixture.fail_once and value["Command"] in ("Draw/SendHttpGif", "Device/PlayTFGif")
                    if fail:
                        fixture.fail_once = False
                result = {"error_code": 1 if fail else 0}
                if value["Command"] == "Device/SaveTFGif" and fixture.fetch_gifs:
                    def download():
                        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                        request = urllib.request.Request(value["NetName"],
                            method="HEAD" if fixture.head_only else "GET")
                        with opener.open(request, timeout=3) as response:
                            data = response.read()
                        if not fixture.head_only:
                            with fixture.lock:
                                fixture.stored_files[value["LocalName"]] = data
                    threading.Thread(target=download, daemon=True).start()
                if value["Command"] == "Device/PlayTFGif":
                    with fixture.lock:
                        if value["FileName"] not in fixture.stored_files:
                            result["error_code"] = 2
                if value["Command"] == "Channel/GetAllConf":
                    result.update(Brightness=35, FirmwareVersion="mock-1")
                if value["Command"] == "Channel/GetIndex":
                    result["SelectIndex"] = 1
                payload = json.dumps(result).encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.http.serve_forever, daemon=True).start()
        self.pipe = rf"\\.\pipe\pixoo-test-{uuid.uuid4()}"
        self.config = root / f"{uuid.uuid4()}.toml"
        transport_line = (
            "" if use_default_transport else f'transport = "{"stored-gif" if storage else "frames"}"\n'
        )
        self.config.write_text(
            f"version = 1\npack = '{pack}'\npipe = '{self.pipe}'\n"
            f"dry_run = {'true' if dry else 'false'}\n[device]\n"
            f'address = "127.0.0.1:{self.http.server_port}"\n{transport_line}'
            "local_token = 123456\nframe_upload_interval_ms = 15\n"
            f"switch_interval_ms = {switch_ms}\ntimeout_ms = 1000\n"
            f"[device.storage]\ncache_dir = '{self.cache}'\nbind = '127.0.0.1:0'\n"
            "settle_ms = 0\ndownload_timeout_ms = 300\n", encoding="utf-8")
        self.log = open(root / f"{uuid.uuid4()}.log", "w+", encoding="utf-8")
        if start:
            self.start()

    def start(self):
        self.process = subprocess.Popen([str(EXE), "run", "-c", str(self.config)],
                                        stdout=subprocess.DEVNULL, stderr=self.log,
                                        creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            self.wait(lambda s: s.get("device", {}).get("last_clip") == "idle")
        except Exception:
            self.close()
            raise

    def close(self):
        if self.process is not None and self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=3)
        self.log.close()
        self.http.shutdown()
        self.http.server_close()

    def state(self):
        result = cli("status", "--pipe", self.pipe, check=False)
        return json.loads(result.stdout) if result.returncode == 0 else {}

    def wait(self, predicate, timeout=5):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            value = self.state()
            if predicate(value):
                return value
            if self.process.poll() is not None:
                self.log.flush()
                self.log.seek(0)
                raise AssertionError(self.log.read())
            time.sleep(.02)
        raise AssertionError(f"state wait timed out: {self.state()}")

    def emit(self, event, session="a", turn="1", **fields):
        value = {"hook_event_name": event, "session_id": session, "turn_id": turn, **fields}
        result = cli("emit", "--pipe", self.pipe, "--strict", data=json.dumps(value))
        assert json.loads(result.stdout) == {}

    def complete(self, session="a", turn="1"):
        cli("notify", "-c", self.config,
            json.dumps({"type": "agent-turn-complete", "thread-id": session, "turn-id": turn}))

    def uploaded(self, name):
        return self.wait(lambda s: s.get("device", {}).get("last_clip") == name)


@unittest.skipUnless(os.name == "nt", "Windows named-pipe integration")
class Integration(unittest.TestCase):
    def setUp(self):
        self.temp_root = Path(__file__).resolve().parents[1] / "local" / "test-tmp"
        self.temp_root.mkdir(parents=True, exist_ok=True)
        # Normal inherited ACLs: Python 3.14's private 0o700 temp-directory ACL
        # prevents an impersonated Windows sandbox identity from accessing it.
        self.root = self.temp_root / f"pixoo-tests-{uuid.uuid4()}"
        self.root.mkdir()
        self.pack = self.root / "pack"
        cli("example", self.pack)
        self.fixtures = []

    def tearDown(self):
        for fixture in self.fixtures:
            fixture.close()
        resolved = self.root.resolve()
        assert resolved.is_relative_to(self.temp_root.resolve()) and not self.root.is_symlink()
        shutil.rmtree(resolved)

    def fixture(self, **options):
        fixture = Fixture(self.root, self.pack, **options)
        self.fixtures.append(fixture)
        return fixture

    def test_default_stored_gifs_preload_once_then_select_without_uploads(self):
        f = self.fixture(storage=True, use_default_transport=True, fail_once=True, switch_ms=0)
        self.assertEqual(f.state()["device"]["transport"], "stored-gif")
        saves = [value for _, value in f.requests if value["Command"] == "Device/SaveTFGif"]
        self.assertEqual(len(saves), len(f.stored_files))
        self.assertGreater(len(saves), 0)
        self.assertTrue(all(data.startswith(b"GIF89a") for data in f.stored_files.values()))
        f.emit("UserPromptSubmit")
        f.uploaded("working-enter")
        f.uploaded("working")
        f.emit("SubagentStart", agent_id="child")
        f.uploaded("delegating")
        f.emit("PreCompact")
        f.uploaded("compacting")
        f.emit("PostCompact")
        f.uploaded("delegating")
        f.emit("SubagentStop", agent_id="child")
        f.uploaded("working")
        f.complete()
        f.uploaded("finished")
        f.uploaded("idle")
        before = len(f.requests)
        time.sleep(.2)
        self.assertEqual(len(f.requests), before, "steady local playback must be silent")
        self.assertEqual(len(saves), sum(value["Command"] == "Device/SaveTFGif" for _, value in f.requests))
        self.assertFalse(any(value["Command"].startswith("Draw/") for _, value in f.requests))
        plays = [value for _, value in f.requests if value["Command"] == "Device/PlayTFGif"]
        self.assertTrue(all(value["FileType"] == 0 and not value["FileName"].startswith("http") for value in plays))
        f.process.terminate()
        f.process.wait(timeout=3)
        f.start()
        self.assertEqual(len(saves), sum(value["Command"] == "Device/SaveTFGif" for _, value in f.requests),
                         "restart must reuse unchanged transfer receipts")

    def test_stored_gif_upsert_compares_bytes_and_force_resends(self):
        f = self.fixture(storage=True, start=False)
        first = json.loads(cli("sync-gifs", "-c", f.config).stdout)["preload"]
        self.assertEqual(first["upserted"], len(f.stored_files))
        second = json.loads(cli("sync-gifs", "-c", f.config).stdout)["preload"]
        self.assertEqual(second["upserted"], 0)
        manifest_path = self.pack / "pet.json"
        manifest = json.loads(manifest_path.read_text())
        manifest["animations"]["idle"]["frame_duration_ms"] = 90
        manifest_path.write_text(json.dumps(manifest))
        changed = json.loads(cli("sync-gifs", "-c", f.config).stdout)["preload"]
        self.assertEqual(changed["upserted"], 1)
        forced = json.loads(cli("sync-gifs", "-c", f.config, "--force").stdout)["preload"]
        self.assertEqual(forced["upserted"], len(f.stored_files))
        self.assertFalse(forced["file_commit_verified"] or forced["playback_verified"])

    def test_save_acknowledgement_or_head_is_insufficient_and_dry_run_stays_offline(self):
        f = self.fixture(storage=True, start=False, fetch_gifs=False)
        prepared = json.loads(cli("sync-gifs", "-c", f.config, "--prepare-only").stdout)
        self.assertGreater(len(prepared["files"]), 0)
        self.assertEqual(f.requests, [])
        self.assertEqual(cli("sync-gifs", "-c", f.config, check=False).returncode, 1)
        self.assertFalse((f.cache / "device-transfers.json").exists())
        f.fetch_gifs = True
        f.head_only = True
        self.assertEqual(cli("sync-gifs", "-c", f.config, check=False).returncode, 1)
        self.assertFalse((f.cache / "device-transfers.json").exists())
        f.head_only = False
        self.assertEqual(cli("sync-gifs", "-c", f.config).returncode, 0)
        dry = self.fixture(storage=True, dry=True)
        self.assertEqual(dry.requests, [])
        self.assertFalse((dry.cache / "device-transfers.json").exists())

    def test_pipe_states_native_loop_and_http_contract(self):
        f = self.fixture(fail_once=True)
        before = len(f.requests)
        time.sleep(.25)
        self.assertEqual(len(f.requests), before, "stable idle must not stream/reupload")
        f.emit("UserPromptSubmit")
        f.uploaded("working-enter")
        f.uploaded("working")
        f.emit("SubagentStart", agent_id="child")
        f.uploaded("delegating")
        f.emit("PreCompact")
        f.uploaded("compacting")
        f.emit("PermissionRequest", tool_name="Bash")
        f.uploaded("needs-input")
        f.emit("PostToolUse", tool_name="mcp__unrelated")
        self.assertEqual(f.state()["state"]["needs_input"], 1)
        f.emit("PostToolUse", tool_name="Bash")
        f.uploaded("compacting")
        f.emit("PostCompact")
        f.uploaded("delegating")
        f.emit("SubagentStop", agent_id="child")
        f.uploaded("working")
        f.emit("UserPromptSubmit", session="b", turn="2")
        f.complete()
        f.wait(lambda s: s.get("state", {}).get("working") == 1)
        self.assertEqual(f.state()["desired_clip"], "working")
        f.emit("Interrupt", session="b", turn="2")
        f.uploaded("interrupted")
        f.uploaded("idle")
        f.emit("SubagentStart", agent_id="late-child")
        self.assertEqual(f.state()["state"]["working"], 0)
        f.emit("UserPromptSubmit", turn="3")
        f.uploaded("working")
        f.complete(turn="3")
        f.uploaded("finished")
        f.uploaded("idle")
        f.emit("SessionEnd")
        f.emit("SessionEnd", session="b")
        f.wait(lambda s: s.get("state", {}).get("sessions") == 0)
        sent = [body for _, body in f.requests if body["Command"] == "Draw/SendHttpGif"]
        self.assertTrue(sent)
        for body in sent:
            self.assertEqual(len(base64.b64decode(body["PicData"])), 64 * 64 * 3)
            self.assertEqual(body["PicWidth"], 64)
            self.assertEqual(body["PicNum"], 4)
            self.assertLess(body["PicOffset"], 4)
            self.assertEqual(body["PicSpeed"], 100)
        self.assertTrue(all(body["LocalToken"] == 123456 for _, body in f.requests))
        self.assertFalse(any(body["Command"] == "Channel/SetBrightness" for _, body in f.requests))
        probe = json.loads(cli("probe", "-c", f.config).stdout)
        self.assertEqual(probe["firmware"], "mock-1")

    def test_delegation_entry_loop_and_exit_use_first_and_last_child(self):
        path = self.pack / "pet.json"
        manifest = json.loads(path.read_text())
        animations = manifest["animations"]
        animations["delegating-start"] = dict(animations["working-enter"])
        animations["delegating-finished"] = dict(animations["finished"])
        animations["delegating"]["entry"] = "delegating-start"
        animations["delegating"]["exit"] = "delegating-finished"
        path.write_text(json.dumps(manifest), encoding="utf-8")
        f = self.fixture()
        f.emit("UserPromptSubmit")
        f.uploaded("working-enter")
        f.uploaded("working")
        f.emit("SubagentStart", agent_id="one")
        f.uploaded("delegating-start")
        f.emit("SubagentStart", agent_id="two")
        f.uploaded("delegating")
        f.emit("SubagentStop", agent_id="one")
        f.wait(lambda s: s.get("state", {}).get("observed_children") == 1)
        self.assertEqual(f.state()["desired_clip"], "delegating")
        f.emit("SubagentStop", agent_id="two")
        f.uploaded("delegating-finished")
        f.uploaded("working")
        self.assertEqual(f.state()["state"]["working"], 1)

    def test_dry_run_and_concurrent_emitters(self):
        f = self.fixture(dry=True)
        def send(index):
            f.emit("UserPromptSubmit", session=f"s{index}", turn=str(index))
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            list(pool.map(send, range(24)))
        f.wait(lambda s: s.get("state", {}).get("working") == 24)
        self.assertEqual(f.requests, [], "dry run must not access the device")

    def test_short_one_shot_return_is_not_delayed_by_switch_pacing(self):
        f = self.fixture(switch_ms=1000)
        f.emit("UserPromptSubmit")
        f.uploaded("working-enter")
        started = time.monotonic()
        status = f.uploaded("working")
        self.assertLess(time.monotonic() - started, .85)
        self.assertGreaterEqual(status["device"]["upload_ms"], 45)

    def test_generated_windows_command_and_existing_notifier_forwarding(self):
        f = self.fixture(dry=True)
        hooks = self.root / "hooks.json"
        cli("hooks", "-c", f.config, "--output", hooks)
        command = json.loads(hooks.read_text())["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
        result = subprocess.run(command, input=json.dumps({"hook_event_name":"UserPromptSubmit",
            "session_id":"a","turn_id":"1"}), text=True, shell=True, capture_output=True,
            creationflags=subprocess.CREATE_NO_WINDOW, timeout=3)
        self.assertEqual(result.returncode,0,result.stderr)
        f.wait(lambda s: s.get("state",{}).get("working")==1)
        output = self.root / "forwarded.json"
        helper = self.root / "notifier.py"
        helper.write_text("import pathlib,sys; pathlib.Path(sys.argv[1]).write_text(sys.argv[2],encoding='utf-8')")
        text = f.config.read_text()
        command = json.dumps([sys.executable, str(helper), str(output)])
        f.config.write_text("notification_forward = "+command+"\n"+text)
        payload=json.dumps({"type":"agent-turn-complete","thread-id":"a","turn-id":"1",
            "last-assistant-message":"private-notifier-message"})
        cli("notify","-c",f.config,payload)
        f.wait(lambda s: s.get("state",{}).get("working")==0)
        end = time.monotonic()+3
        while not output.exists() and time.monotonic()<end: time.sleep(.02)
        self.assertEqual(output.read_text(encoding='utf-8'),payload)
        self.assertNotIn("private-notifier-message",json.dumps(f.state()))

    def test_absent_bridge_and_invalid_hook_are_harmless(self):
        for data in ["not-json", json.dumps({"hook_event_name": "Stop"}),
                     json.dumps({"hook_event_name": "UserPromptSubmit", "session_id": "a"})]:
            result = cli("emit", "--pipe", rf"\\.\pipe\absent-{uuid.uuid4()}", data=data)
            self.assertEqual(json.loads(result.stdout), {})
            self.assertEqual(result.stderr, "")

    def test_reusable_setup_preserves_settings_notifier_and_detects_edits(self):
        repo = Path(__file__).resolve().parents[1]
        home = self.root / "codex home"
        home.mkdir()
        settings = home / "config.toml"
        hooks_path = home / "hooks.json"
        settings.write_text("# keep this comment\nnotify = ['existing.exe', 'done']\nmodel = 'stable'\n[features]\nother = true\n", encoding="utf-8")
        unrelated = {"hooks":[{"type":"command","command":"security-check"}]}
        hooks_path.write_text(json.dumps({"other":True,"hooks":{"UserPromptSubmit":[unrelated]}}), encoding="utf-8")
        bridge = self.root / "bridge config.toml"
        bridge.write_text(f"# private bridge\npack = '{self.pack}'\n[device]\naddress = '192.0.2.1'\n", encoding="utf-8-sig")
        before = [p.read_bytes() for p in (settings, hooks_path, bridge)]

        def ps(script, *args, check=True):
            result = subprocess.run(["powershell", "-NoProfile", "-File", str(repo/"scripts"/script),
                *map(str,args)], text=True,capture_output=True,check=False,timeout=15,
                creationflags=subprocess.CREATE_NO_WINDOW)
            if check: self.assertEqual(result.returncode,0,result.stderr)
            return result

        staged = self.root / "review"
        ps("prepare-codex-setup.ps1", "-Config", bridge, "-Executable", EXE,
            "-CodexHome", home, "-OutputDirectory", staged)
        self.assertEqual(before, [p.read_bytes() for p in (settings, hooks_path, bridge)])
        ps("apply-codex-setup.ps1", "-PlanDirectory", staged)
        self.assertEqual(before, [p.read_bytes() for p in (settings, hooks_path, bridge)])
        ps("apply-codex-setup.ps1", "-PlanDirectory", staged, "-Apply")
        c = tomllib.loads(settings.read_text(encoding="utf-8"))
        b = tomllib.loads(bridge.read_text(encoding="utf-8"))
        self.assertEqual(c["model"],"stable")
        self.assertTrue(c["features"]["other"] and c["features"]["hooks"])
        self.assertIn("# keep this comment",settings.read_text())
        self.assertIn("# private bridge",bridge.read_text())
        self.assertEqual(b["notification_forward"],["existing.exe","done"])
        self.assertEqual(c["notify"][1:3],["notify","--config"])
        merged = json.loads(hooks_path.read_text())
        self.assertEqual(merged["hooks"]["UserPromptSubmit"][0],unrelated)
        self.assertEqual(len(merged["hooks"]["UserPromptSubmit"]),2)
        self.assertTrue(merged["other"])
        backups=list(staged.glob("backup-*"))
        self.assertEqual(len(backups),1)
        self.assertEqual([ (backups[0]/name).read_bytes() for name in ("config.toml","hooks.json","bridge.toml") ],before)
        repeat=self.root/"review-again"
        ps("prepare-codex-setup.ps1", "-Config", bridge, "-Executable", EXE,
            "-CodexHome", home, "-OutputDirectory", repeat)
        self.assertEqual(json.loads((repeat/"hooks.json").read_text()), merged)
        self.assertEqual(tomllib.loads((repeat/"bridge.toml").read_text())["notification_forward"],b["notification_forward"])
        settings.write_text(settings.read_text()+"\n# concurrent edit\n",encoding="utf-8")
        saved = hooks_path.read_bytes()
        rejected=ps("apply-codex-setup.ps1", "-PlanDirectory",repeat,"-Apply",check=False)
        self.assertNotEqual(rejected.returncode,0)
        self.assertIn("Settings changed since",rejected.stderr)
        self.assertEqual(saved,hooks_path.read_bytes())

        # A fresh Codex home has no existing files to back up or overwrite.
        fresh = self.root/"new home"
        new_plan=self.root/"new-home-review"
        ps("prepare-codex-setup.ps1", "-Config",bridge,"-Executable",EXE,
            "-CodexHome",fresh,"-OutputDirectory",new_plan)
        self.assertFalse(fresh.exists())
        ps("apply-codex-setup.ps1", "-PlanDirectory",new_plan,"-Apply")
        self.assertTrue((fresh/"hooks.json").is_file())
        self.assertTrue(tomllib.loads((fresh/"config.toml").read_text())["features"]["hooks"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path)
    args, rest = parser.parse_known_args()
    if args.exe:
        EXE = args.exe.resolve()
    unittest.main(argv=[__file__, *rest])
