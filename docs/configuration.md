# Bridge configuration

See [the example](../config/bridge.example.toml). The bridge reads TOML once at startup; restart it after edits. Unknown fields and invalid bounds are rejected. An IP/hostname reservation on your router can keep `address` stable.

| Field | Default / purpose |
| --- | --- |
| version | 1 |
| pack | `pets/default`; directory containing `pet.json`, relative to the config |
| pipe | `\\.\pipe\pixoo-pet`; use a unique name for separate bridges/devices |
| dry_run | false; simulate playback and perform no device requests |
| notification_forward | Empty command array; put any previous Codex `notify` array here |
| device.address | Required for live operation, a host or host:port without `http://` or `/post` |
| device.transport | `stored-gif`; upsert new/changed GIFs at startup and select local filenames; `frames` explicitly enables per-switch RGB uploads |
| device.local_token | Optional integer; sent as LocalToken, never included in status output |
| device.brightness | Optional 0-100; omitted preserves the display setting |
| device.max_frames | 40, configurable down to 1 |
| device.timeout_ms | 3000, range 100-30000 per HTTP request |
| device.frame_upload_interval_ms | 150; wait between upload frames in `frames` mode, not between playback frames; unused in stored mode |
| device.switch_interval_ms | 0; immediate ordinary clip selection; positive values add pacing; one-shot expiry may return sooner |
| device.playback_start_delay_ms | 0; measured visible-start offset added to the stored-GIF request time, or frame-upload completion time |
| animations | Logical state → supplied clip name; missing mapping uses the same name, empty string disables it |
| idle_alternates.animations | Empty list by default, disabling alternates; clip names to choose from at random |
| idle_alternates.min_interval_ms | 45000; minimum uninterrupted normal-idle wait |
| idle_alternates.max_interval_ms | 60000; maximum wait, chosen uniformly with inclusive bounds |
| idle_alternates.avoid_immediate_repeat | true; exclude the previous alternate when at least two are configured |

Use TOML literal single-quoted strings for Windows paths and named pipes, avoiding backslash escaping. For example:

```toml
pack = 'C:\my-pets\decker'
pipe = '\\.\pipe\pixoo-pet'
notification_forward = ['C:\existing\notifier.exe', 'turn-ended']

[device]
address = '192.168.1.50'

[animations]
working = 'typing-fast'
finished = ''
```

Keep credentials and absolute personal paths in `local/`, which is ignored. A pack stays portable and does not contain an IP/token. Status reports expose clip names, counts, prepared bytes, device health and upload duration, not device credentials or conversation text.

The [standard example](../config/bridge.example.toml) uses stored playback and the [complete alternate-idle pack](../pets/decker/revisions/idle-alternates-v2/combined/pet.json), preserving the delegation-v3 lifecycle artwork. Configs that omit `device.transport` also use stored playback. An existing explicit `transport = "frames"` setting keeps per-switch RGB uploads. See [preloading and local playback](stored-gifs.md) for commands and verification details.

## Stored GIF settings

| Stored-GIF field | Default / purpose |
| --- | --- |
| device.storage.cache_dir | `gif-cache` relative to config; native GIF exports and exact-byte transfer receipts |
| device.storage.directory | `codex-pet`; relative device folder, using letters, numbers, hyphens, underscores and `/` |
| device.storage.bind | `0.0.0.0:8787`; temporary GIF download server during changed-file preload |
| device.storage.public_base_url | Optional HTTP URL; otherwise infer the host interface from its route to the Pixoo |
| device.storage.download_timeout_ms | 15000, range 100-60000; wait for a complete device GET after save acknowledgement |
| device.storage.settle_ms | 500, range 0-10000; allow device file writing after the observed transfer |

Stored-GIF status includes `storage.phase`, transfer counts, `device.last_played_clip` and `device.selection_ms`. Transfer receipts record completed host transfers, not verified device file contents. Use `sync-gifs --force` after clearing/replacing the device's storage or changing the physical device at the same address. Stored playback never silently switches to frame uploads.

HTTP requests bypass proxies and redirects and go directly to the configured LAN device. Keep this local API on your trusted network. The named pipe rejects remote clients and grants the current Windows user and logon access, including the restricted token used by Codex's Windows sandbox.

## Alternate idle animations

```toml
[idle_alternates]
animations = ["idle-flyby", "idle-yawn", "idle-message", "idle-city"]
min_interval_ms = 45000
max_interval_ms = 60000
avoid_immediate_repeat = true
```

The wait starts when the device writer acknowledges normal idle. Harmless events that leave the bridge idle do not extend it. At expiry, the bridge chooses one listed clip, plays a single pass, returns to normal idle, and chooses a fresh wait after that return is acknowledged. Each approved alternate has forty frames at the same 83 ms clock as the other animations and lasts 3.320 seconds. Alternate clips never chain back to back.

Work, attention, compaction and observed child activity cancel the wait and preempt a playing alternate. The bridge starts a full new wait when normal idle resumes; idle time is not accumulated across interruptions. Finish/interruption reactions also complete before the next wait starts. The scheduler uses the aggregate logical state even when an optional priority clip is absent.

Set `animations = []` or omit the table to disable alternates. Set both interval values equal for a fixed wait, or set `avoid_immediate_repeat = false` to allow consecutive selections of the same clip. Bounds must satisfy `100 <= min_interval_ms <= max_interval_ms <= 86400000`. The configured idle must exist and loop (or have one frame). Alternate references must exist, resolve to distinct clips, and differ from normal idle; invalid settings fail before playback. Each alternate is treated as a single pass even if its source has `loop = true`. Its entry/exit references are not played.

`status` includes the resolved alternate names, interval settings, playing alternate and `next_in_ms`. The remaining time is a snapshot from the controller's last update; the bridge sleeps until an event or timer instead of polling. Stored mode preloads alternate files at startup along with the lifecycle clips. A live startup can still be preparing GIFs after IPC becomes available.

Validate without sending device requests:

```powershell
.\local\bridge-build\release\pixoo-pet.exe check-config --config .\local\bridge.toml
```

## Rebuild and restart from source

Run [rebuild-bridge.ps1](../scripts/rebuild-bridge.ps1) in your PowerShell from the repository:

```powershell
.\scripts\rebuild-bridge.ps1 -Config local\bridge.toml
```

The script tests the bridge, runs Clippy, checks its PowerShell guards, builds into `local/bridge-build`, and validates the config/pack before touching the installed executable. It selects the executable the same way as `start-bridge.ps1`: a repository-root `pixoo-pet.exe` if present, otherwise `target/release/pixoo-pet.exe`. Use `-Executable` when the hooks use another installed path. The Rust MSVC toolchain and C++ build tools are required.

For the first alternate-idle update, the prepared settings are beside the existing configs as `bridge.idle-alternates.pending.toml` and `bridge-stored-gifs.idle-alternates.pending.toml`, each with a `.plan.json` hash receipt. The script automatically uses the pending file matching `-Config`, checks for intervening edits, installs the compatible executable before the new settings, and archives the staging files after successful startup. This ordering keeps the old completion adapter working until the executable is replaced. Device settings and the original notification command are preserved.

Backups and logs go under `local/bridge-restarts/<timestamp>/`. Only a running bridge with that exact executable and absolute config path is stopped. A shared executable running another config, or a relative config that cannot be verified, causes the script to stop before replacement. It preserves an existing command-line `--dry-run` override, starts the new bridge hidden and confirms its process ID through the named pipe. On installation/startup failure it attempts to restore the previous files and restart the previous process. Startup confirmation means IPC is ready; inspect `storage.phase`/`device.healthy` while new GIFs preload.

Use `-CheckOnly` to build and validate without stopping, installing or starting a bridge. `-SkipTests` explicitly skips the Rust/Clippy/PowerShell tests while still building and validating. A later settings edit to the active config requires a restart because settings are read once. To update the other private config, use `-Config local\bridge-stored-gifs.toml` after the active bridge has stopped using the shared executable. Python is optional; additional mock-device CLI checks can be run with:

```powershell
python tests\integration.py --exe local\bridge-build\release\pixoo-pet.exe -v
```
