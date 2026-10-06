# Idle revision 3

This candidate uses the owner's preferred cell from the generated typing grid: first column, second row, index `004`. The [new native base](../../base-v3.png) exactly matches that extracted cell's RGB pixels. The owner liked the previous idle revision and confirmed that it corrected the monitor and hand movement issues; this revision changes the base and background activity.

The original [source sheet](../idle-v2/sources/typing-poses.png) is 1254x1254, so its sixteen cells were not literal 64x64 files. The bridge's sheet export reduces the complete 4x4 grid to 256x256 using nearest-neighbor sampling and extracts each native 64x64 cell. [base-selection.json](base-selection.json) records the chosen cell and export. The [base preview](../../previews/base-v3.png) enlarges the actual 64x64 pixels without smoothing. No creative regeneration of the chosen base was needed.

Preview: [enlarged GIF](review/idle-preview.gif), [native GIF](review/idle-native.gif), [contact sheet](review/contact.png), and [hand crops](review/hands-contact.png). [review/pet.json](review/pet.json) and [review/idle-sprite.png](review/idle-sprite.png) form a standalone idle-only pack. On 2026-10-03, the owner requested a physical preview, and the bridge was started with a separate private preview config pointing to this pack. The normal config is preserved; the display remains on idle for review.

## Motion

- 32 frames at 83 ms, approximately 12 FPS and 2.656 seconds per loop. There are 30 distinct complete frames, with intentional holds between typing key poses.
- The selected base's face, visor, room and monitor housings remain fixed. Five far-hand and eight near-hand fingertip pixels can flex on alternating presses; palm anchors and the surrounding hand outlines remain unchanged.
- The main monitor contents scroll within its new interior mask. The old base's mask coordinates are not reused.
- A generated green oscilloscope trace scrolls within the rear monitor instead of a blinking cursor. The saved raster trace is reduced using maximum color sampling to retain its narrow green stroke at 11x7 pixels, then composited over the fixed screen background.
- Two 4x2-pixel cars cross the distant window on separate paths, moving in opposite directions. The second car uses a horizontal mirror of the existing generated car. Both begin and end outside the visible window.

The shared GIF palette keeps fixed colors consistent across frames. GIF delays distribute 80/90 ms ticks, for 2.650 seconds total. Native PNG sprite frames retain full RGB color and uniform 83 ms timing.

## Sources and reproduction

The [waveform source](sources/oscilloscope-wave.png) was created with Codex's built-in image generation using the saved [prompt](sources/oscilloscope-prompt.md). The chosen base, typing poses and car come from the previously saved [idle-v2 sources](../idle-v2/sources/). The Rust authoring example performs deterministic registration, masking, mirroring, scrolling and exports. No image generation or Python runtime is required to rebuild the saved artwork or run the bridge.

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example idle_revision -- pets/decker/revisions/idle-v3/authoring.json pets/decker/revisions/idle-v3/build-next
.\target\release\pixoo-pet.exe validate pets/decker/revisions/idle-v3/build-next
```

[authoring.json](authoring.json) specifies the scene masks, palm anchors, typing timeline, waveform and car paths. The example retains compatibility with idle-v2's cursor and single-car plan; all 32 previous idle-v2 frames were regenerated and compared with the saved revision, with identical RGB pixels.

The new pack passes bridge validation, and the authoring example passes Clippy with warnings denied. [verification.json](review/verification.json) records independent decoding checks: 3786 fixed pixels exactly match the selected base, the face and palm anchors remain unchanged, car sprites are mirrored and their paths oppose, the waveform has eleven visible phases, and both GIF exports preserve fixed pixels and timing. The last-to-first changes are confined to the two screen interiors. The actual Pixoo accepted all 32 frame commands, with a 10.671-second upload. The owner approved its appearance and animation on the panel, choosing this base and the 32-frame guidance for the subsequent [work revision](../work-v3/README.md).
