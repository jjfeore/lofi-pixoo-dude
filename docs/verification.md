# Verification record

Tested on the owner's Windows computer on 2026-10-02, using a stable Rust 1.99 MSVC release build. Codex desktop was observed as 26.930.2377.0 with a bundled 0.159.0-alpha.12.1 CLI. These are local observations, not claims about every Codex version or Pixoo firmware.

## Code and packaging

- Fifteen Rust tests pass for metadata normalization, parallel-chat priority, matching approval resumption, child-stop behavior, terminal fences, partial packs, RGB/alpha conversion, asset paths, import, timing validation, masked sheet export, and setup merging.
- Clippy passes with warnings denied for all targets.
- All six Windows integration tests passed against the final release in 12.4 seconds. They exercise actual named pipes and cover concurrent emitters, valid no-op hook output, absent-bridge behavior, generated shell commands, original notifier forwarding, HTTP upload/retry and native-loop silence, all selected states, stale events, and one-shot return pacing.
- Setup integration uses isolated Codex homes. It verifies unchanged destinations during preparation/check-only, comments and unrelated settings/handlers retained, exact backups, existing notifier preservation, duplicate-free updates, rejection of intervening edits, and initial installation when no settings files exist. No global settings are written by the test harness.
- The reusable skill passes the system skill validator. The full personal pack validates before playback; every source cell is 64x64 RGB and each concrete clip contains eight frames at 83 ms. The idle/working compaction variants are resolved separately.

Repeatable checks from source:

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
python tests\integration.py -v
python scripts\benchmark.py --pack pets\decker --seconds 20
```

Python is only a development harness. The executable and PowerShell setup helpers do not require it.

## Host performance

The full personal pack holds 1,196,032 bytes of prepared frame payloads. The final 4,338,688-byte release executable was measured with 100 direct emitter launches and 100 Windows shell invocations, followed by a 20-second quiet bridge sample:

| Measurement | Observed |
| --- | --- |
| Direct process median / p95 | 22.6 / 30.7 ms |
| Shell invocation median / p95 | 52.5 / 67.4 ms |
| Resident / private memory | 8.6 / 2.4 MiB |
| Quiet bridge CPU delta | 0 measurable seconds in 20 seconds |

Final maxima were 49 ms direct and 109 ms through the shell. An earlier full-pack sample had 24/40 ms direct/shell medians and a single 430 ms direct launch outlier. The emitter's 50 ms IPC budget does not bound process launch or OS scheduling. A short zero-measured sample is not proof of zero CPU under all loads. These numbers include neither LLM time nor Pixoo upload delay. Do not describe Rust as eliminating hook overhead.

Stable loops cause no continuing host HTTP frame stream. The mock-device test verifies no reupload while the desired clip is unchanged. Memory scales with all prepared clips, and PostToolUse resumption still launches an emitter for matched tools even without a pending approval.

## Device and live integration

Readonly LAN status requests succeeded with and without the owner's optional local token. The device reported brightness 100 and selected channel 1; its firmware version was not returned. A real-device diagnostic sequence exercised idle, working entry/work, compaction, delegation, needs-input, interruption and return to idle. Each upload returned acceptance. Four-frame uploads took about 1.48-1.83 seconds including configured pacing. The original channel was restored after that diagnostic test.

Acceptance is evidence of the HTTP contract, not a camera or owner verification of LEDs, visible start time, exact 12 FPS playback, or seamless single-play transitions. The bridge maintains one current raw clip; loading the next complete clip can expose repeated gesture frames. See [limitations](limitations.md).

The finished personal pack was then exercised through the running bridge with an explicit metadata replay. All nine concrete clips were accepted, including both compaction poses, with 17 uploads and correct state bookkeeping/return to idle. Eight-frame uploads had a 3.19-second median, 2.87-second minimum and 5.95-second maximum. The bridge was left running with the owner's private configuration. The owner confirmed that the actual display plays and animates correctly, while requesting artwork refinements after the first implementation commit. Exact optical timing and seamless single-play transitions were not independently measured.

The owner installed the proposed merged global settings and reported trusting the new commands. Installed hook groups and completion adapter were checked against the prepared merge; existing GSD handlers and the original notifier were preserved. Automatic/manual compaction, every approval route, subagent continuation and startup reconstruction have not been exhaustively proven in live desktop chats. The deterministic test suite verifies their documented event mappings; it does not create a richer universal waiting-state API.

Local raw measurement records, device status and private configuration are under ignored `local/`. Release bundles omit that directory and contain no device token or personal Codex settings.
