# Pixoo64 playback latency investigation

Investigated on the owner's display on **2026-10-05**. The task was to determine whether animation states can be uploaded once and selected quickly enough to justify entry/exit clips.

## Finding

There is now positive evidence for native GIF-frame retrieval: requests for frames 90 and 60 of the proposed saved vendor GIF displayed still Iron Man and Hulk images matching those exact source frames. The earlier Decker idle appearance in the ordinary gallery also shows retention without a loading screen. However, this investigation did **not** establish reliable filename selection between multiple retained Decker clips, whole-loop switching latency, or survival across a deliberate reboot.

The failed Decker file-import trials have a concrete setup problem: neither a phone nor another PC could reach the GIF server. The probe process was confirmed to be inside Windows AppContainer isolation, and Windows denied firewall/network-profile inspection. This does not prove that AppContainer is the only cause, but it means the zero-download results cannot establish that the Pixoo's save operation is unsupported.

The current raw-frame transport remains too slow for short transitions. Reducing its additional frame pauses helps, but does not remove the loading screen. Preserve the transition artwork and retry native file imports from a reachable server before deciding the hardware cannot switch quickly enough.

## Current bridge behavior

`src/device.rs` calls `Draw/ResetHttpGifId`, then sends every prepared RGB frame sequentially with `Draw/SendHttpGif`, using `PicID: 1`. Between responses it waits for `device.frame_upload_interval_ms`, which defaults to 150 ms. Both the owner's configuration and the example use that default.

For a 32-frame animation, those 31 waits alone total **4.65 seconds**. HTTP transfer and device processing add more time. The ordinary switch interval is an additional, separate pacing constraint. Stable playback itself requires no continuing host frame stream.

The display's upload IDs are documented as an increasing transfer sequence, not documented resident-animation handles. See Divoom's [ID documentation](https://docin.divoom-gz.com/web/#/5/55) and [frame upload documentation](https://docin.divoom-gz.com/web/#/5/57).

## Measurements on this display

The test clips came from `pets/decker/revisions/work-v12/review`. Each raw clip contains 32 full 64×64 RGB frames, with an 83 ms playback interval.

| Test | Additional wait between frames | Time to finish upload |
| --- | --- | --- |
| Idle | 150 ms | 10.731 seconds |
| Idle | 0 ms | 5.905 seconds |
| Working | 0 ms | 6.230 seconds |
| Working, recovery upload | 0 ms | 6.373 seconds |

These times include reset, sequential requests and configured pauses. They end at the last successful HTTP response; they are not optical measurements of the LEDs. Each frame request was accepted. The owner observed loading before both idle and working during the zero-added-wait replay, followed by an immediate return to the built-in gallery rotation.

Zero added wait is not a concurrent upload: the next request still waits for the previous response. This limited experiment does not establish reliability under prolonged switching.

A 32-frame transition lasts about 2.66 seconds. With the measured raw transport, uploading a transition and then its steady loop can cost roughly **12 seconds of transfer**, in addition to the gesture. Fewer frames reduce that transfer cost at the expense of motion detail or playback cadence.

## Storage and GIF playback tests

Divoom documents several relevant commands:

- [Save a GIF locally](https://docin.divoom-gz.com/web/#/5/356): `Device/SaveTFGif`, with a source URL in `NetName` and a destination path in `LocalName`.
- [Play a GIF](https://doc.divoom-gz.com/web/#/12?page_id=195): the older documentation lists local files, folders and network URLs. The [newer page](https://docin.divoom-gz.com/web/#/5/54) only documents network URLs (`FileType: 2`). These documents differ; neither is a guarantee for this device.
- [Display a stored GIF frame](https://docin.divoom-gz.com/web/#/5/357): `Device/PlaySomeFrameGif`, with `FileName` and `FrameId`. This is a single-frame command, not proof of complete local-loop selection. It was not used to claim successful playback.
- [Select a custom page](https://docin.divoom-gz.com/web/#/5/32): `Channel/SetCustomPageIndex`, with `CustomPageIndex` from 0 through 2.

The device reports hardware type **92** through Divoom's LAN discovery service. Its local settings response did not report a firmware version. The presence of a removable TF/microSD slot was not confirmed.

The following experiments were performed:

1. A temporary LAN HTTP server served only the prepared 64×64 GIFs. Tests used both port 8765 and port 80. Server self-checks returned the correct GIF bytes.
2. `Device/SaveTFGif` returned success, but no requests from the Pixoo reached the server during the observed windows. Saving to the proposed paths was therefore not verified.
3. `Device/PlayTFGif` with network URLs returned success, but likewise produced no device fetches. On an announced GIF-only retry, the owner confirmed the default gallery continued rather than showing Decker.
4. Local-file playback returned success. Because those files had not been confirmed saved, this was not an independent test of successful-file playback or a definitive rejection of local-file support.
5. A vendor-hosted 64×64 test GIF was checked for dimensions and passed to the URL command. The command returned success; visible playback was not confirmed. This avoids relying solely on the LAN server, but does not establish the firmware's capability.
6. Reusing the previously uploaded raw ID with only its first or last frame packet returned success, but the owner reported loading or freezing. This did not provide a usable recall operation. A complete normal upload was then sent to clear any partial sequence, and gallery channel 1 was restored successfully.
7. The device's cloud upload and liked-image lists were empty, so they supplied no addressable file ID for the observed Decker idle clip. Several local configuration/custom-list commands supplied no useful enumeration.

The tests distinguish **accepted commands** from **successful downloads and playback**. The LAN GIF path was unusable in these initial trials. Subsequent phone/PC checks confirmed that the test server was not reachable from other LAN devices; local self-checks had concealed this limitation. The exact network policy responsible remains unresolved. Hardware and firmware differences also remain possible causes. Do not conclude that the display has no storage or can never play custom resident animations.

Historical firmware strings also show separate HTTP, local and remote image sources, cache-management functions, and native gallery/configuration paths. That historical binary is not a dump of the owner's current firmware and does not establish a callable cache-selection contract.

## Next native-file test

Run the narrow server from a normal PowerShell session outside the agent's AppContainer:

```powershell
& 'C:\Users\jjfeo\Repos\lofi-pixoo-dude\local\playback-research\serve-gifs.ps1'
```

It serves only the four prepared 64×64 GIFs, records request sources and completed server writes under ignored `local/playback-research/`, and stops automatically after 15 minutes or with Ctrl+C. It changes no firewall settings. Confirm that another LAN device can fetch `http://192.168.1.18/idle.gif` while the server is live; the address must be rechecked if the computer's LAN address changes. Running outside AppContainer is the next diagnostic step, not a guarantee that ordinary Windows firewall rules permit inbound traffic.

The probe now supports an existing external server, avoiding a conflicting port-80 listener:

```powershell
& 'C:\Users\jjfeo\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe' -X utf8 'C:\Users\jjfeo\Repos\lofi-pixoo-dude\local\playback-research\storage-probe.py' native-save --external-server http://192.168.1.18
```

This requests four saves under `codex_native_probe/`: idle, working, working-enter and finished. **Verify requests from the Pixoo's address and completed GIF writes in the standalone server's log before treating the files as downloaded.** Server write completion is transfer evidence, not proof that the device committed a file or can play it.

Only after those downloads are confirmed, use the same probe with `native-switch` instead of `native-save`. It requests idle, working, idle and working again through `Device/PlayTFGif`, type 0, with eight-second observation windows and no RGB-frame uploads. Confirm distinct loops, repeated recall, steady playback and visible Loading behavior. Test the single-play clips separately once loop switching works. Neither of these new phases was run against a reachable external server in this investigation.

The helper passed local verification of exact idle/working bytes, content lengths, HEAD handling, denial of unrelated paths and automatic shutdown; the PowerShell launcher parsed without errors. These checks validate the helper, not inbound LAN access.

## Custom-page fallback

Use the Divoom app to preload **one idle loop on custom page 0 and one working loop on custom page 1**, with one animation per page. Then test repeated page selection using:

```json
{"Command":"Channel/SetCustomPageIndex","CustomPageIndex":0}
```

and page 1. No raw-frame upload should accompany these selections. Check that each page holds the selected loop instead of rotating elsewhere, switches without loading, and can be recalled repeatedly. Restore the original channel after the experiment.

This is a documented alternative to arbitrary local GIF paths, but app preloading and Decker page selection were **not performed** here. Three selectable custom pages may be useful for idle, working and needs-input. That alone does not address all bridge states or both transition directions; individual gallery-member selection would still need investigation.

A [recent Home Assistant implementation](https://github.com/gickowtf/pixoo-homeassistant/pull/158) reports successful hosted-GIF switching without loading on another Pixoo64. Its hardware observations support further testing, not a claim that this owner's display behaves the same way. It also reports that opaque delta GIFs with a shared palette and disposal mode 1 avoid artifacts seen with other encodings.

Prepared import candidates are under ignored `local/playback-research/`:

| File | Size | Intended use |
| --- | --- | --- |
| `import-idle.gif` | 32,937 bytes | Loop on custom page 0 |
| `import-working.gif` | 59,806 bytes | Loop on custom page 1 |
| `import-working-enter.gif` | 98,423 bytes | Later transition experiments |
| `import-finished.gif` | 96,800 bytes | Later transition experiments |

These are actual 64×64 GIFs, not the enlarged review previews. They use an opaque shared 256-color palette and delta frames with disposal mode 1. GIF timing is in centiseconds; 80/90 ms intervals preserve the approximately 83 ms mean. Adjacent identical idle frames coalesce, retaining their combined duration. Palette conversion is not lossless RGB. Dimensions, decoding, timing, disposal, transparency and loop metadata were checked; device/app import and visual appearance remain unverified. The original artwork was not modified.

## Web documentation follow-up

The owner's three repository links were checked against their actual source on 2026-10-05. This follow-up performed source research only; it did not send another playback command to the display.

| Source | What the implementation actually does | Evidence for resident selection |
| --- | --- | --- |
| [4ch1m/pixoo-rest](https://github.com/4ch1m/pixoo-rest/blob/master/pixoo_rest/helpers.py#L43) | `handle_gif()` resets the HTTP GIF ID, decodes the GIF on the computer, and posts each RGB frame using `Draw/SendHttpGif`. Its pass-through endpoints also expose custom-page selection. | No save-once/file-recall method was found in the reviewed routes and helpers. |
| [tidyhf/Pixoo64-Advanced-Tools](https://github.com/tidyhf/Pixoo64-Advanced-Tools/blob/main/pixoo1664/__init__.py#L51) | `send_images()` builds one `Draw/CommandList` containing every raw RGB frame. Both local GIF playback and its cloud-artwork UI decode and upload frames. `set_custom_channel()` selects a custom page and channel 3. | Its GIF and cloud-artwork paths are uploads rather than stored-file selection. The three custom pages remain a relevant lead. |
| [cyanheads/pixoo-toolkit](https://github.com/cyanheads/pixoo-toolkit/blob/main/src/client.ts#L465) | `playGifUrl()` sends `Device/PlayTFGif`, `FileType: 2`. The current [CLAUDE.md](https://github.com/cyanheads/pixoo-toolkit/blob/main/CLAUDE.md#L107) describes types 0/1 but explicitly says the client has no SD-card method. | The implementation establishes URL playback, not local-file recall. Its current documentation distinguishes the APK's `Sys/PlayTFGif` name from the HTTP API's `Device/PlayTFGif`. |

The Advanced Tools batching implementation conflicts with [Grayda's hardware notes](https://github.com/Grayda/pixoo_api/blob/main/NOTES.md) and the toolkit documentation, which say multi-frame `Draw/CommandList` does not work. This is a separate, unverified transfer-speed experiment; it is not evidence of storage playback or a reason to change the bridge without a device test.

### Official storage commands and documentation conflict

The official pages were read through Showdoc's page-content API, because their browser URLs expose a JavaScript shell to text-only readers. Local snapshots preserve the returned content and metadata under `local/playback-research/sources/divoom-docs/`.

- The [older Pixoo64 play-GIF page](https://doc.divoom-gz.com/web/#/12?page_id=195), whose returned metadata reports 2022-01-19, documents `FileType: 0` for a TF-card file, `1` for a TF-card folder, and `2` for a URL.
- The [current Pixoo64 play-GIF page](https://docin.divoom-gz.com/web/#/5/54), whose returned metadata reports 2026-09-16, documents only type `2` and describes other values as errors. It includes hardware 90/92 and 96 endpoint instructions. Both versions refer to implementation after firmware version 90096; the owner's firmware version remains unknown.
- Nevertheless, the current Pixoo64 catalog also documents [saving a GIF locally](https://docin.divoom-gz.com/web/#/5/356) and [displaying a frame from a local GIF](https://docin.divoom-gz.com/web/#/5/357). The documentation therefore does not settle which local-file operations work on the owner's current firmware.

On firmware that supports both saving and whole-file recall, the documented requests would be:

```json
{"Command":"Device/SaveTFGif","NetName":"http://<host>/idle.gif","LocalName":"codex/idle.gif"}
```

then, after independently confirming that saving completed:

```json
{"Command":"Device/PlayTFGif","FileType":0,"FileName":"codex/idle.gif"}
```

Folder playback uses `FileType: 1` and a folder path, which rotates a collection rather than naming an individual state. These examples are documented candidate requests, not successful results on the owner's device. In particular, a raw-frame `PicID` does not supply a TF-card filename, and the prior successful HTTP acknowledgements did not confirm that the proposed local files existed.

The separate local-frame request is:

```json
{"Command":"Device/PlaySomeFrameGif","FrameId":10,"FileName":"codex/idle.gif"}
```

It requires an existing file and is not documented as a whole-animation replay command. No indexing convention beyond the example was established.

### Additional implementation references

- [Grayda/pixoo_api](https://github.com/Grayda/pixoo_api/blob/main/pixooapi/pixoo.py#L1723) actually sends `Device/PlayTFGif` for its SD-file, SD-folder and URL modes. Its [enum](https://github.com/Grayda/pixoo_api/blob/main/pixooapi/types.py#L71) maps them to 0, 1 and 2. The separately named `LOCALFILE` mode means a file on the computer decoded and uploaded as frames. This is an older wrapper, not a current hardware compatibility test.
- [r12f/divoom](https://github.com/r12f/divoom/blob/main/divoom/src/divoom_contracts/pixoo/animation/api_play_gif_file.md) documents all three source types, and its [request implementation](https://github.com/r12f/divoom/blob/main/divoom/src/divoom_contracts/pixoo/animation/api_play_gif_file.rs) sends the corresponding `FileType` and `FileName`. It references the older Divoom page, so it corroborates the protocol rather than resolving the newer documentation conflict.
- [eugene-bert/divoom-golang-api](https://github.com/eugene-bert/divoom-golang-api/blob/main/device.go#L136) has misleading TF-card descriptions: `PlayTFGif()` hard-codes type 2, while `SaveTFGif()` supplies neither `NetName` nor `LocalName`. Its descriptions should not be treated as a verified way to save the current raw animation or recall a resident file.
- The [Home Assistant hosted-animation change](https://github.com/gickowtf/pixoo-homeassistant/pull/158) supplies recent developer-reported playback without a loading screen, using type 2 and a GIF fetched from an HTTP server. This is evidence that a faster file transfer path works on another device, not evidence of arbitrary resident-cache selection on this display.

Two native selection candidates remain worth testing after app preloading:

1. Put exactly one Decker loop in each of two custom pages and select them with `Channel/SetCustomPageIndex` (indices 0–2). The Advanced Tools wrapper also explicitly selects channel 3 after the page. This is the most direct documented way to test resident state switching without having to discover a local filename.
2. Import/upload clips through the Divoom app, obtain their cloud `FileId` values using the upload/like list APIs, and select them with the official [Draw/SendRemote](https://docin.divoom-gz.com/web/#/5/63) request. The command selects a Divoom gallery asset; the documentation does not promise local cache hits or a latency bound. Repeated switching would need to establish those properties.

No reviewed source provides a documented enumeration-and-recall operation for the particular raw-uploaded Decker clip seen in the default rotation. The gallery observation remains evidence of retention without an established selection handle. Repository revisions used for this follow-up are recorded in `local/playback-research/sources/revisions.json`.

## File and folder playback follow-up

At the owner's request, `local/playback-research/storage-probe.py` ran additional bounded storage trials on 2026-10-05. Each phase recorded and restored its initial channel. The first playback phases temporarily selected gallery channel 1; later controls started directly from clock channel 0. The initial channel for these trials was clock channel 0. Unlike the earlier stored-path test, this follow-up explicitly tried folder playback and several plausible path conventions.

| Operation | Cases | Device responses |
| --- | --- | --- |
| `Device/SaveTFGif` | Idle/working at root, in `codex_store_probe/`, and in `divoom_gif/`; vendor-hosted 64×64 GIF at root | All 7 accepted |
| `Device/PlayTFGif`, type 0 | Six Decker paths, vendor save path, the documentation's `divoom_gif/1.gif`, and an absolute `/sdcard/` file path | All 9 accepted |
| `Device/PlayTFGif`, type 1 | Probe/native GIF folders with and without trailing slashes, plus absolute `/sdcard/` folder paths | All 6 accepted |
| `Sys/PlayTFGif` | Type 0, type 1, and a filename-only request without `FileType` | All 3 accepted |
| `Sys/SaveTFGif` | Source URL and a separate Codex-owned destination | Explicitly rejected as `Request data illegal json` |

The server used port 80 and served the smaller prepared native GIFs (32,937 and 59,806 bytes). Six LAN save requests were accepted, but **zero requests from the Pixoo reached the GIF server** during the observed windows. The host self-check fetched the correct bytes. The vendor-hosted source was independently downloaded and verified as a 64×64, 120-frame GIF, but the Pixoo's download from that external server could not be observed here. None of the save destinations was independently confirmed to exist.

The owner reported that **Decker did not appear**, but an apparent error/test pattern appeared for about ten seconds at a time. This is evidence of visible behavior during the trials; exact correspondence to individual requests was not established. A failed file lookup is a plausible explanation, but the pattern alone does not identify the cause or prove that the retained gallery clip can be addressed through these paths.

The `Sys/PlayTFGif` acknowledgements are a useful discovery: the name is accepted by this unit's HTTP endpoint despite its absence from the current official HTTP documentation. Acknowledgement still does not establish successful file playback. The failed `Sys/SaveTFGif` response also shows that unrecognized commands do not uniformly return success.

Detailed results are the timestamped `*-storage-save.json`, `*-storage-files.json`, `*-storage-folders.json`, `*-storage-aliases.json`, and `*-storage-paths.json` files under ignored `local/playback-research/`. A separate control trial compares the proposed saved vendor filename, a deliberately nonexistent filename, and direct vendor URL playback. Its visual interpretation is tracked separately rather than assuming that the observed pattern was an error.

The vendor control was then run twice. Stored and deliberately missing filenames both returned success; direct vendor URL requests both timed out after five seconds. On the first run, subsequent read requests temporarily received connection-refused errors, then channel restoration succeeded. The second run also recovered and a final read confirmed clock channel 0. The owner observed an Avengers animation, followed by a Divoom-logo animation, then a persistent clock. The vendor GIF does contain the Avengers characters, but the tests first selected the ordinary gallery, so this observation alone cannot distinguish stored-file recall from gallery playback. The logo and temporary API interruption are consistent with a restart, but the cause was not established. Further direct requests to this vendor URL were avoided after the repeated interruption.

The subsequent `clock-control` trial requested the proposed saved vendor filename for 12 seconds directly from clock mode, restored the clock for five seconds, then requested a unique nonexistent filename for ten seconds. The owner reported clock, Avengers, a default "HOT" image, then clock. An isolated `missing-only` trial then started from the clock and requested only another unique nonexistent filename for 15 seconds. **The owner again saw the default "HOT" image.** The missing request was accepted in 102.63 ms; the final read and restoration both confirmed clock channel 0.

This negative control establishes that a nonexistent filename can produce a default image. Consequently, an accepted request followed by recognizable gallery artwork does not establish that a saved file exists or that its filename selected that artwork. The Avengers observation remains inconclusive until saving and distinct file selection are independently verified. The downloaded vendor GIF has 120 frames of 30 ms each and an infinite-loop extension; a normal end-of-file loop setting does not explain the later unrelated image, although device decoding behavior remains unverified.

### Indexed-frame result and host networking

The `vendor-frames` phase then requested `Device/PlaySomeFrameGif` from `codex_store_probe_vendor.gif`, first with `FrameId: 90` and then `FrameId: 60`, restoring the clock between requests. The owner confirmed **still Iron Man, clock, still Hulk, clock**, matching the two indexed frames of the downloaded source. The requests returned success in 112.59 and 117.69 ms. These are command response times, not measured LED start times. The owner also reported a still image for the nonexistent-file control, but did not identify that image. Thus the positive result supports GIF-frame retrieval while leaving filename fallback behavior unresolved. It does not establish switching between two different Decker files.

While the LAN server was temporarily live, the owner tried its idle URL from both a phone and another PC on the network; neither could reach it. The server recorded no remote connection. A read-only Windows token query returned `is_app_container: true` and `is_elevated: false`. Firewall COM inspection failed with `E_ACCESSDENIED`; network-profile/TCP-listener inspection failed because CIM access was unavailable. Microsoft documents [AppContainer network isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation), including separate grants for acting as a server. The observed host isolation is a plausible explanation for the failed inbound transfers; no firewall policy was read successfully or changed.

GET requests for the Codex-owned vendor filename at root, `/sdcard/`, and the native GIF-folder prefixes all returned HTTP 404. This establishes that those HTTP paths do not expose the file; it does not establish that the file is absent from device storage.

The published Decker GIF previews checked in the existing public repository were 512×512. They were not sent to the native GIF API, which documents support only for 16, 32 and 64 pixels. No new animation was uploaded publicly as a workaround.

### Protocol controls without display observation

After the owner stepped away, deliberately nonexistent short command names were sent in each of the `Device`, `Channel`, `Draw` and `Sys` namespaces. **All four were explicitly rejected as `Request data illegal json`.** Consequently, the accepted save/play operation names are meaningful evidence of recognized requests; the endpoint does not acknowledge every arbitrary name. Recognition still does not certify a successful asynchronous download, lookup or rendering operation.

`Channel/GetCustomPageIndex` returned only `error_code: 0`, without a page index. No page-selection changes or content preloads were sent in that control, because the original page could not be read back. This does not disprove the documented page-selection operation; app-preloaded page content and visible recall remain untested.

A batching probe checked whether `Draw/GetHttpGifId` could validate completed uploads without an observer. Two consecutive reads were stable; reset returned ID 0, and a normally acknowledged two-frame upload also left the reported ID at 0. The counter therefore supplied no distinguishing completion signal in this trial. **No `Draw/CommandList` upload was sent**, and batching was not validated or rejected. A complete ordinary 32-frame working upload cleared partial state, taking 7.304 seconds without added frame pauses; all requests were accepted and clock channel 0 was restored. That last upload was not optically observed.

Relevant additional logs are `*-storage-vendor-frames.json`, `*-storage-connectivity.json`, `stored-file-readback.json`, `short-unknown-command-control.json`, `*-page-control.json`, `*-batch-control.json`, and `gif-server-self-check.json`. The frame trial's settings read reported clock/gallery times of 0 rather than the preceding 60; the probes issued no explicit writes to those timing fields. Channel restoration was verified, while complete preservation of device settings was not claimed.

## Practical bridge decisions

- Reducing `frame_upload_interval_ms` is an available improvement to calibrate on the device, but the zero-wait experiment still took approximately six seconds per 32-frame upload.
- Omit the working entry and finish reaction while using this raw transport if loading dominates the gestures.
- Keep the artwork while testing native storage. Matching indexed frames and recognized save/play commands justify a reachable-server retry; the failed LAN trials are not evidence that six-second loading is unavoidable.
- To disable entry with the current runtime, remove the `entry` reference from the working clip in a separate playback manifest. Setting only an `animations.working-enter` mapping to an empty string does **not** disable that direct manifest lookup. A finish reaction can be disabled with `finished = ''` under `[animations]` in bridge configuration.
- Do not implement a resident transport from HTTP success responses alone. Verify upload/preload, independent clip selection, repeat selection, visual loading behavior and single-play behavior first.

The production configuration, runtime source and animation authoring files were left untouched. The bridge's usual live pipe was absent during the probes. The latest follow-up read confirmed a reachable device on clock channel 0, and restoration to that channel succeeded.

Detailed command logs, timing results, probe scripts, import candidates and historical firmware evidence are in ignored `local/playback-research/`. No device token is included in those command logs.
