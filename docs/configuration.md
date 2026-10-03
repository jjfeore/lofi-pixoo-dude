# Bridge configuration

See [the example](../config/bridge.example.toml). The bridge reads TOML once at startup; restart it after edits. Unknown fields and invalid bounds are rejected. An IP/hostname reservation on your router can keep `address` stable.

| Field | Default / purpose |
| --- | --- |
| version | 1 |
| pack | `pets/default`; directory containing `pet.json`, relative to the config |
| pipe | `\\.\pipe\pixoo-pet`; use a unique name for separate bridges/devices |
| dry_run | false; simulate uploads and perform no device requests |
| notification_forward | Empty command array; put any previous Codex `notify` array here |
| device.address | Required for live operation, a host or host:port without `http://` or `/post` |
| device.local_token | Optional integer; sent as LocalToken, never included in status output |
| device.brightness | Optional 0-100; omitted preserves the display setting |
| device.max_frames | 40, configurable down to 1 |
| device.timeout_ms | 3000, range 100-30000 per HTTP request |
| device.frame_upload_interval_ms | 150; wait between upload frames, not between playback frames |
| device.switch_interval_ms | 1000; ordinary clip upload pacing; one-shot expiry may return sooner |
| device.playback_start_delay_ms | 0; extra host hold time after upload for a measured visible-start delay |
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

HTTP requests bypass proxies and redirects and go directly to the configured LAN device. Keep this local API on your trusted network. The named pipe rejects remote clients and grants the current Windows user and logon access, including the restricted token used by Codex's Windows sandbox.
