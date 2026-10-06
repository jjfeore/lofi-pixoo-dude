# Needs input with amber indicators

This first candidate is superseded by [needs-input-v3](../needs-input-v3/README.md), which restores typing and replaces the rear exclamation mark with an animated warning triangle at the owner's request. Its pulse, visor, full main-glass motion and traffic are preserved.

The [preview](review/needs-input-preview.gif) has 32 native 64x64 frames at 83 ms, approximately 12 FPS and 2.656 seconds per loop. It uses the approved base and lowered working visor. The hands rest at the keyboard while the main screen scrolls, the visor light and monitor gently breathe amber, and a persistent exclamation mark appears on the rear display. Two opposing traffic lanes carry a distant cyan car and a larger magenta car.

The [review pack](review/pet.json) adds `needs-input` to the accepted compaction pack. All six previous physical clips, their seven logical selections, source frames, sprites, individual GIF previews and motion metadata are preserved exactly. Delegation and interruption still await separate artwork revisions. This pack has not been activated on the device; owner visual review is pending.

## Motion and source

The offline [compaction compositor](../../../../examples/compaction_revision.rs) has an optional `attention` mode controlled by [authoring.json](authoring.json). It reuses the accepted working frames and existing vehicle images. Only the full main-screen aperture, powered visor pixels, thirteen fingertip pixels, rear-screen contents and clipped sky traffic can change. No character or room regeneration was needed.

All 209 main-glass pixels retain scrolling content, including after the brightness pulse is removed from the measurement. The pulse runs once through the full loop and ranges from 64% to 100%; neither the symbol nor visor light disappears. Sixteen actual lens/indicator pixels turn amber, including a dim blue edge identified during native inspection. The remaining visor housing stays fixed. The thirteen typing pixels use the resting working frame throughout, keeping both hands on the keyboard without gestures.

The rear display uses a simple code-native geometric `!`: a broad stem, a separate dot and a dark gap. It remains readable throughout the cycle. The traffic profile gives each of the two cars one pass per loop, in opposite directions at different heights and phases. [Full-scene contacts](review/needs-input-contact.png), [rear-screen crops](review/needs-input-rear-contact.png) and the [poster](review/needs-input-poster.png) support native visual inspection.

Only the new needs-input GIFs use a newly fitted global palette to include amber colors. A shared palette keeps fixed scene colors stable within the loop; minor quantization differences from earlier GIF palettes are possible. The device sprite source remains full RGB PNG. Previously accepted GIFs are copied unchanged.

## Reproduce and verify

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example compaction_revision -- pets/decker/revisions/needs-input-v2/authoring.json local/needs-input-v2-rebuild
python pets/decker/revisions/needs-input-v2/verify.py local/needs-input-v2-rebuild
```

Compilation and export succeeded. [Independent saved-image verification](review/verification.json) passed fixed body/hand and bezel regions, fixed visor housing, amber indicator colors, absence of duplicate powered forehead glow, full-glass content motion, the persistent symbol, traffic on both lanes, sprite ordering, fixed GIF colors and native/enlarged GIF matching. All 32 complete frames are distinct. Pixel-region comparisons establish preservation against the accepted source; they are not an anatomy classifier.

The optional compositor change also reproduced the previous compaction pack's 192 PNG frames and twelve individual GIF files exactly, together with its sprites, posters, contacts, manifest, motion metadata and cycle previews. Saved motion information is in [attention-motion.json](review/attention-motion.json).

GIF previews distribute 80/90 ms ticks for a 2.650-second loop. Native sprite playback retains uniform 83 ms frame timing.

## Remaining tool checks

The sandbox rejected Rust formatting with `Access is denied`, Clippy with `failed to read CARGO_MANIFEST_DIR as a directory: Access is denied`, and bridge validation with `pack directory not found: Access is denied`. These checks were attempted and are not marked passed. Run the existing [external helper](../../../../scripts/check-artwork.ps1) in your PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example compaction_revision -Pack pets/decker/revisions/needs-input-v2/review -Format
```

The helper formats the offline example, checks Clippy with warnings denied and validates the pack. It does not upload artwork, change device settings or rebuild the bridge runtime.
