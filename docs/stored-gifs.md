# Preloading and local GIF playback

Stored GIFs are the default transport, including when `device.transport` is omitted. At startup the bridge exports the configured animations as native 64x64 GIFs, upserts changed GIFs using `Device/SaveTFGif`, and selects them using `Device/PlayTFGif` with `FileType: 0`. Runtime selections send a local filename; they send no GIF bytes, `Draw/ResetHttpGifId` or `Draw/SendHttpGif` packets. `device.switch_interval_ms` defaults to 0 for immediate selection.

The [standard example config](../config/bridge.example.toml) and [stored-GIF example](../config/bridge.stored-gif.example.toml) both point at the complete [delegation-v3 pack](../pets/decker/revisions/delegation-v3/review/pet.json). Copy the example to `local/bridge.toml` and preserve your device address, optional local token and existing `notification_forward` array. Relative pack/cache paths are resolved from the config's directory.

```powershell
.\target\release\pixoo-pet.exe sync-gifs --config .\local\bridge.toml --prepare-only
.\target\release\pixoo-pet.exe sync-gifs --config .\local\bridge.toml
.\target\release\pixoo-pet.exe run --config .\local\bridge.toml
```

`run` performs synchronization automatically; a separate `sync-gifs` command is optional. Normal hidden startup uses `scripts/start-bridge.ps1`, which loads `local/bridge.toml`. To opt into the older frame-upload transport, explicitly set `transport = "frames"` in `[device]`; `switch_interval_ms = 1000` restores its former ordinary selection pacing.

Run in your own PowerShell when the Codex sandbox cannot read directory metadata or accept incoming LAN connections. The preload server must be reachable from the Pixoo at the advertised HTTP URL. `storage.public_base_url` can override interface detection; its host/port must reach the temporary server specified by `storage.bind`. The server exposes only the exported GIFs, accepts the configured device's resolved IP addresses, and closes after synchronization. It does not serve repository files.

On startup the bridge checks the exact encoded GIF bytes against its prior transfer receipts. Unchanged files skip saving, including after a bridge restart. Changed files are saved under the same owned device filenames. Receipts are scoped to the endpoint and device folder and are updated after each completed transfer, so a partial sync resumes without repeating its completed files. `sync-gifs --force` resends everything. No device files are deleted.

The export follows configured state mappings, both compaction variants, entry/exit references and the default animation. Unused and disabled clips are omitted except where another configured clip or default references them. Names in stored mode must be at most 64 ASCII letters, numbers, hyphens or underscores. Derived one-frame hold GIFs cover one-shots that need to retain their final pose when no base is supplied. One-shots still return under the bridge's timer, using the GIF's rounded playback duration plus the configured visible-start delay. Additional helpers do not replay delegation entry; the last observed helper stop selects delegation exit.

GIF timing is rounded cumulatively to 10 ms units, preserving the approximate 83 ms cadence. All frames share one opaque palette. Up to 256 source colors are represented exactly; larger color sets are quantized. The source pack is unchanged.

Saving has two required observations: a successful save response and a complete GET from the configured device to the temporary GIF server. A successful response alone, a HEAD request or a missing download does not create a receipt. A failed run-time preload keeps the IPC/state engine responsive while retrying with bounded backoff; `sync-gifs` exits with a diagnostic instead. Events received during preload select the latest state when storage becomes ready. Reactions older than ten seconds are skipped as before.

## Device verification

Divoom's [save-GIF documentation](https://docin.divoom-gz.com/web/#/5/356) describes saving a downloaded file locally. Its [older playback documentation](https://doc.divoom-gz.com/web/#/12?page_id=195) describes type 0 local-file playback, while its [current playback page](https://docin.divoom-gz.com/web/#/5/54) lists only type 2 URL playback. This firmware-dependent conflict remains unresolved by a successful HTTP response.

A completed server transfer establishes that the GIF was served to the device. The API has no verified file listing, content readback or storage-commit acknowledgement in this bridge. The status/report therefore keeps `file_commit_verified` and `playback_verified` false. Existing transfer receipts assume the device retained the files; clear/replaced storage requires `--force`.

The owner confirmed repeated idle → working → idle → working filename selection on this display, reporting instant switches with no visible loading. All 16 configured GIF transfers have receipts in the private cache. This is a visual check of those two loops, not an optical latency measurement or a delegation/one-shot timing check. Before relying on another device, verify distinct clips, repeated recall, delegation entry/loop/exit, one-shot return timing and visible loading behavior. The earlier [hardware investigation](playback-investigation.md) records the preceding import/connectivity trials. The `frames` transport remains available explicitly for firmware that cannot recall local GIFs.
