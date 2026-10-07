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

The [standard example](../config/bridge.example.toml) uses stored playback and the complete delegation-v3 review pack. Configs that omit `device.transport` also use stored playback. An existing explicit `transport = "frames"` setting keeps per-switch RGB uploads. See [preloading and local playback](stored-gifs.md) for commands and verification details.

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
