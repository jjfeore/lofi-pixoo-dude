# Alternate idle animation candidates

Four review candidates for occasional vignettes during uninterrupted idle time. Each is forty native 64x64 RGB frames at 100 ms per frame, lasting four seconds. The review manifest marks them as single-play clips. The GIFs repeat to make visual review convenient.

| Candidate | Changes |
| --- | --- |
| [Window flyby](review/idle-flyby-preview.gif) | A larger rose hovercar passes close to the window; a cyan sweep crosses the rear monitor. |
| [Sleepy Decker](review/idle-yawn-preview.gif) | The eye closes, the mouth opens and the chin drops slightly for a yawn; the light behind Decker warms to amber and fades back. |
| [Incoming message](review/idle-message-preview.gif) | An envelope notification slides onto the rear monitor; a mint header and message rows scroll over the entire main glass. |
| [Night city](review/idle-city-preview.gif) | A lit elevator descends; three building floors light up briefly; the light behind Decker pulses through a rainbow. |

The [four-way GIF](review/alternates-overview.gif) uses that order: flyby and yawn across the top, message and city across the bottom. [Key poses](review/key-poses.png) show each sequence at 0, 1, 2 and 3 seconds. Individual native GIFs, enlarged GIFs, contact sheets, numbered PNGs and native sprite sheets are included in `review/`.

## Preserved artwork

All candidates reuse the accepted idle from `delegation-v3/review`, including its fingertip motion, visor, full main-screen scrolling, traffic and rear waveform. Ambient motion continues forward over one complete cycle. Every pixel outside each candidate's declared layers matches its corresponding idle source frame. The first and last frames match the accepted idle start exactly. The normal idle PNGs, sprite sheet and GIFs are copied byte-for-byte.

The new hovercar and expression study were made with the built-in imagegen tool. Original generated images and the exact prompts are in [sources](sources/prompts.json). The hovercar is normalized once and clipped behind the window frame and foreground character. The closed eyelid is extracted from the expression study. The lower face reuses the existing `interrupted-v4` mouth/chin rasters, whose neutral lower-face pixels match this idle exactly. This avoids the generated study's faint mouth and preserves the reviewed one/two-pixel chin drop. No arm motion or new body poses are introduced.

## Export and verification

The [offline example](../../../../examples/idle_alternates.rs) and [authoring plan](authoring.json) reproduce the candidates. Choose a fresh output directory:

```powershell
cargo run --locked --offline --release --example idle_alternates -- pets/decker/revisions/idle-alternates-v1/authoring.json local/idle-alternates-rebuild
python pets/decker/revisions/idle-alternates-v1/verify.py local/idle-alternates-rebuild
```

The read-only verifier checks frame dimensions/count/order, single-play manifest flags, four-second timing, actual changes in every intended layer, yawn eyelid/mouth visibility, exact source pixels outside those layers, canonical endpoints, byte-identical normal idle exports, and agreement between native and enlarged GIFs. GIF timing is exactly 100 ms with no timing normalization needed. Each new GIF uses a fixed palette across its full sequence; the PNG/sprite sources retain the native RGB pixels.

The release example compiled and the artwork verification passed. Direct Rust formatting and bridge `validate` encounter the checkout's Windows sandbox directory-access limitation; their outcomes are recorded separately from image checks. No device playback was attempted for these candidates.

The comparison preview uses losslessly compressed RGB sprite sheets and offers synchronized playback, pause, scrubbing and an actual-size view. JavaScript syntax and those controls passed offline checks in [preview-controls-verification.json](review/preview-controls-verification.json). A browser render check could not reach the sandbox's local preview server, so browser rendering is not recorded as verified.

## Proposed scheduling after approval

On entry into idle, choose a fresh random deadline 45-60 seconds away. At the deadline, choose an approved alternate, preferably avoiding an immediate repeat, play it once and return to normal idle. Choose the next 45-60 second deadline when the vignette completes. Leaving idle cancels the pending deadline; new work or a higher-priority state immediately preempts a vignette. Use monotonic elapsed time so clock changes cannot alter the wait.

This revision supplies artwork for approval. The live pack, bridge state machine, configuration, process and device storage are unchanged. Random idle scheduling remains to be implemented after selection of the approved candidates.
