# Idle revision 2

The selected candidate is in [review](review/). It is a standalone idle-only pack for review, using the previously approved [64x64 base](../../base-v2.png). The running bridge and the original full pack have not been changed.

The owner reviewed this revision positively and confirmed that it corrected the reported monitor and hand motion issues. [Idle revision 3](../idle-v3/README.md) follows it with a newly selected base, two opposing cars and an oscilloscope trace.

Preview: [enlarged GIF](review/idle-preview.gif), [native 64x64 GIF](review/idle-native.gif), [32-frame contact sheet](review/contact.png), and [enlarged hand crops](review/hands-contact.png). The bridge source is [idle-sprite.png](review/idle-sprite.png) with [pet.json](review/pet.json).

## Motion

- 32 frames at 83 ms, approximately 12 FPS and 2.656 seconds per loop. Sixteen generated typing key poses are scheduled with two-frame holds; 28 complete exported frames are distinct. This is one continuous timeline, not a repeated eight-frame loop.
- Main monitor contents scroll inside a fixed five-by-sixteen-pixel aperture. The monitor housing, screen outline, stand and perspective stay identical to the approved base.
- Far and near fingertip presses alternate. Four far-hand pixels and ten near-hand pixels can change; wrists, palms, the gap between the hands and surrounding keyboard remain fixed.
- A two-pixel cursor blinks on the rear monitor. A small generated hovercar passes across a narrow strip of the distant window, leaving the window frame and skyline fixed. The car is outside the visible window at both ends of the loop.

The GIFs use one shared palette to avoid frame-to-frame changes in the fixed scene's colors. GIF delay units are 10 ms, so the previews distribute 80/90 ms delays and total 2.650 seconds. The bridge's PNG frames retain full RGB color and uniform 83 ms timing.

## Reproduce

Creative raster sources were produced with Codex's built-in image generation, then registered, masked and exported by the offline Rust authoring example. The exact prompts and source images are saved under [sources](sources/): [typing prompt](sources/typing-prompt.md), [typing grid](sources/typing-poses.png), [car prompt](sources/flying-car-prompt.md), and [transparent car](sources/flying-car.png). No image-generation API key is needed to consume or rebuild the saved sources.

Run from the repository root, choosing a new output directory:

```powershell
cargo run --locked --release --example idle_revision -- pets/decker/revisions/idle-v2/authoring.json pets/decker/revisions/idle-v2/build-next
.\target\release\pixoo-pet.exe validate pets/decker/revisions/idle-v2/build-next
```

[authoring.json](authoring.json) specifies the masks, palm anchors, key-pose schedule, screen aperture, cursor and car path. The generated cells have some camera and hand drift; only the registered fingertip pixels are consumed. All frames start from the approved base. The masks are specific to this scene.

The selected pack passed the bridge validator and the authoring example passed Clippy with warnings denied. [verification.json](review/verification.json) records an independent check of the exported PNGs and decoded GIFs, including fixed pixels, palm anchors, timing and loop endpoints. These checks establish geometry and export consistency; the visual quality of the typing remains for owner review. This 32-frame candidate has not been uploaded to the Pixoo, so its device loading time has not been measured.
