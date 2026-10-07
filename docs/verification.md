# Verification record

## Absolute one-shot deadlines, 2026-10-07

Review of the brief repeat found a host timing error: the writer waited for the local-file play response, then the runtime started a fresh full-duration countdown after receiving its acknowledgement. The updated writer anchors stored playback to the start of the successful play request, after any brightness setup. Frame playback retains its upload-completion anchor. The writer and runtime share the resulting absolute deadline, including the existing calibrated visible-start offset. A late response or queued acknowledgement does not restart the countdown, and an expired one-shot bypasses ordinary switch pacing when returning.

All nine storage tests and eight in-memory runtime tests passed. Four new regressions cover delayed local-file responses and prior brightness setup, expired-deadline return with two-second switch pacing, completed frame-upload timing, delayed acknowledgement delivery, stale serials and native-loop silence. Existing deterministic GIF encoding, configured exports/upserts and six delegation selection tests continue to pass. The existing filesystem-based runtime fixture was excluded because directory canonicalization is blocked in this managed Windows session. These checks establish the host timing behavior; visible playback after installing this update remains to be confirmed by the owner.

The release compiled and linked, but Cargo could not replace the running installed executable. The fresh compiled artifact was staged under `local/animation-timing-fix-20261007/`; its source/artifact timestamps, Windows executable header, distinct SHA256, version command, update-helper PowerShell syntax and installer check-only hash validation passed. The guarded helper backs up and replaces the executable, then restarts the exact existing bridge configuration. The owner must run it in normal PowerShell. No hook, configuration, shortcut or encoded-GIF changes are needed for this update.

## Natural delegation retest, 2026-10-07

After the owner applied the executable update, its installed SHA256 matched the prepared candidate. A fresh, read-only subagent task exercised the real lifecycle hooks without synthetic bridge events. The running bridge log recorded AgentStart with one observed child, delegation entry and loop, then AgentStop with zero observed children, delegation exit and return to working. The parent turn was held open until the exit animation's timer completed. The owner confirmed that the delegation animations displayed correctly, and reported that work entry/completion clips briefly began a second pass before returning. This establishes natural start/stop event delivery and visible delegation playback; the repeat observation prompted the absolute-deadline timing change.

## Delegation parent-session handling, 2026-10-06

The owner confirmed natural prompt and interruption animations after installing and trusting the corrected Windows hooks. The bridge log recorded working entry/work, completion/idle and interruption/idle. A real short subagent task then left the display in working; no delegation selection appeared in the log. Read-only session metadata confirmed that the subagent had its own turn ID. The exact SubagentStart/SubagentStop stdin payload was not captured, so event receipt on that failed live test is still unconfirmed.

A new regression test demonstrated that the engine rejected delegation hooks with child turn IDs. The fix tracks children under their parent session without replacing the parent's active turn, accepts absent child turn IDs, and keeps late child hooks from restarting a completed parent. All eleven engine tests and all six delegation playback tests passed. The playback tests now use distinct child turn IDs and cover first/last-child entry/exit, parallel helpers, optional/missing transitions, higher-priority states and interruption. Received delegation hooks log only event kind and aggregate child count, making receipt visible without conversation content or identifiers. Python integration syntax, the prepared update helper's syntax, and Git whitespace checks passed. Physical delegation playback with the updated executable remains unverified.

The updated release compiled and linked, but replacing the running executable failed with access denied. The compiled candidate was saved under `local/delegation-turn-fix-20261006/`; its version command, new diagnostic marker, distinct binary hash and installer check-only validation passed. The guarded owner-side installer backs up the existing executable and restarts only the bridge using the exact configured executable and `local/bridge.toml`. It does not change hook definitions or trust. The targeted CLI delegation integration test against the candidate failed during `example` fixture creation with the existing sandbox directory restriction; its event/playback assertions were not reached.

## Windows hook launch fix, 2026-10-05

The running bridge logged its initial idle selection, but no prompt or interruption event was observed during the owner's live chat test. The installed Pixoo hook definitions had matching trust hashes. Their command used a quoted executable path without an invocation operator; reproducing that command in the session's PowerShell failed with a parser error before launching the emitter. The current Codex hook runner inherits the session shell.

Generated hooks now include `commandWindows`. Safe bare paths invoke the emitter directly in either Windows shell; paths requiring quoting use an encoded PowerShell invocation. Three targeted setup tests passed, including actual PowerShell and cmd.exe launches with spaces, apostrophes, literal shell characters and preserved JSON stdin. A separate native-emitter probe through both shells reached the expected missing diagnostic pipe error, without sending events to the live bridge. A hook-only apply plan passed its hash guards and check-only validation; it preserves unrelated handlers, existing configuration and notification forwarding. Applying it and reviewing/trusting the changed handlers through Codex remain owner actions because global settings are outside the agent's writable roots. Natural event delivery and visible transitions after installation remain unverified.

The updated release compiled and linked, but installing it over the running `target/release/pixoo-pet.exe` failed with access denied. The compiled executable was saved as `local/pixoo-pet-powershell-hooks.exe` and its version command succeeded. The hook-only plan invokes the existing emitter, so it does not require replacing or restarting the running bridge. Clippy and rustfmt remain blocked by sandbox directory metadata access. The targeted CLI integration test against the compiled candidate failed during `example` fixture creation for the same filesystem restriction; its shell and notifier assertions were not reached. Python syntax and Git whitespace checks passed.

## Default stored-GIF playback, 2026-10-05

Stored GIFs are now the default for omitted transport settings and the standard example, with `switch_interval_ms = 0`. The normal private `local/bridge.toml` was migrated to the tested delegation-v3 configuration, retaining the original device credentials, pipe, notification forwarding and unrelated settings. Its cache and device folder reuse matching exact-byte transfer receipts for all 16 files. The original config was backed up under ignored `local/`. Explicit `transport = "frames"` remains available.

The owner ran the bounded local-file test after preloading the GIFs: idle → working → idle → working, six seconds per clip. The owner reported instant switches with no visible loading. This confirms repeated recall of those two stored loops on this display; delegation entry/loop/exit and host-timed one-shot returns were not covered by that visual test. No optical latency measurement or reboot-persistence test was performed.

The current binary unit run reported 29 passed and the same four filesystem failures caused by sandbox directory canonicalization (`Access is denied`, os error 5). The new omitted-transport/explicit-frames regression passed. All seven storage tests passed using the implicit transport default, including upsert/receipt reuse and writer selections for transitions and holds without event-time uploads. Python integration-test syntax, example/private config consistency, PowerShell helper syntax and Git whitespace checks passed. Clippy and rustfmt still fail on sandbox directory metadata access.

The updated release executable built successfully. The targeted default-transport CLI/named-pipe integration test was attempted against it, but failed during fixture setup because the `example` command encounters the same directory access restriction; its playback assertions were not reached. Run `scripts/check-stored-gifs.ps1 -Format` in the owner's PowerShell for the remaining full host checks. No live bridge was started during this defaults change.

## Initial stored-GIF implementation, 2026-10-05

The selectable stored-GIF transport exports native GIFs, upserts changed files before playback, and uses type 0 local filename selection for runtime changes. Seven storage tests passed in the managed Windows session. They cover deterministic encoding, exact pixels for a limited palette, native dimensions/timing/repeat metadata, configured reference selection, all 11 physical delegation-v3 clips plus five derived holds, complete-GET requirements, unchanged/changed/forced sync behavior, invalidated receipts after failed forced resync, and transition/hold playback without raw-frame uploads.

The full binary unit suite reported 28 passed and four failed. Those same four filesystem tests failed before this change because directory canonicalization returns `Access is denied (os error 5)` in this sandbox. The existing release pack validator fails at the same directory step. Clippy and rustfmt are also blocked by directory metadata access, and the CLI/named-pipe integration additions remain unexecuted here. Python test syntax and the PowerShell helper syntax passed. `git diff --check` passed.

A read-only probe using the prepared stored-GIF configuration confirmed that the Pixoo was reachable, at brightness 100 and clock channel 0; firmware was not returned. No stored-GIF preload or visual playback was verified on the physical device in this update. Use `scripts/check-stored-gifs.ps1 -Format -Sync` from the owner's PowerShell to perform the remaining full checks and preload, then verify visible local-file recall. The private `local/bridge-stored-gifs.toml` selects delegation-v3 and preserves the current device credentials and completion forwarding. The existing active configuration was retained.

## Initial implementation, 2026-10-02

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
