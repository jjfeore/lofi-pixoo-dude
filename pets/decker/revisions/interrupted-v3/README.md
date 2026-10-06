# Interrupted: stable mouth opening and bolder shock rays

Superseded by [interrupted-v4](../interrupted-v4/README.md), which adds a small jaw drop and return synchronized with this mouth opening. Owner review found v3 almost perfect but requested that additional chin motion.

The [revised preview](review/interrupted-preview.gif) contains forty native 64x64 frames at 83 ms, approximately 12 FPS and 3.320 seconds. The mouth opens for a shout, holds briefly, then closes. Three bolder red rays burst from the visor. The chase, red screen corruption, rear halt symbol, automatic visor lift and recovery remain as in interrupted-v2.

The [context preview](review/interruption-cycle-preview.gif) shows working, interruption and idle together. It is a 104-frame desktop preview; the device interruption has forty frames and `loop: false`. Its first frame equals the accepted working start and its last equals the accepted idle start.

## Focused artwork revision

[authoring.json](authoring.json) configures the editable [Rust compositor](../../../../examples/interruption_revision.rs). The generated jaw patches are disabled. Only eight mouth-interior pixels change: a dark opening grows vertically, with upper teeth and a small tongue accent. The opening reaches its full size at frame 3, holds through frame 6 and closes by frame 9. The cheek, chin and surrounding face stay fixed; there is no face translation or shrinking jaw.

Each ray has a four-pixel bright core and four-pixel red edge, making three two-pixel-thick rays totaling 24 pixels. They flash in frames 2, 4 and 6 along the existing shock timeline. These small shapes and the mouth aperture are authored directly in the existing compositor. No new generated raster is used; the earlier expression study remains in v2 as historical material.

All 4,008 pixels outside the former jaw mask and new ray area match v2 in every corresponding frame. Both arms, hands and typing retain their approved source pixels. The main glass still animates over all 209 visible pixels, with a fixed bezel. One getaway and five patrols move forward with alternating police lights, while the rear screen and visor recover to idle.

## Reproduce and verify

From the repository root, choose a fresh output directory:

```powershell
cargo run --locked --release --example interruption_revision -- pets/decker/revisions/interrupted-v3/authoring.json local/interrupted-v3-rebuild
python pets/decker/revisions/interrupted-v3/verify.py local/interrupted-v3-rebuild
```

Compilation, export and [independent PNG/GIF verification](review/verification.json) passed. Checks cover the mouth stages and stable surrounding face, ray colors, forty-frame timing, canonical endpoints, full-glass motion, fixed hands/bezel, chase and recovery, sprite ordering and GIF colors. All seven prior physical clips are preserved exactly: 224 native frames and fourteen individual GIFs. A separate reproduction of v2 also kept all its PNG/GIF exports, manifests and motion records byte-identical, including 264 native clip frames and sixteen individual GIFs. Native scene, head and rear contacts were inspected; owner review of v3 is pending.

Rustfmt, Clippy and bridge CLI pack validation were attempted but blocked by sandbox directory-access errors. To complete those checks in your PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example interruption_revision -Pack pets/decker/revisions/interrupted-v3/review -Format
```

This revision is a separate review pack. No bridge runtime build, live activation, settings change, device upload or commit occurred.
