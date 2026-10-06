# Typing with an animated warning triangle

The [32-frame needs-input preview](review/needs-input-preview.gif) restores the exact typing motion from the accepted working loop. The rear screen shows a filled amber warning triangle with a dark exclamation mark. A soft highlight moves through the triangle while its outline stays fixed. The previous gentle brightness pulse remains exactly the same, and the symbol stays visible throughout.

The loop uses native 64x64 frames at 83 ms, approximately 12 FPS and 2.656 seconds. The lowered amber visor, scrolling across all 209 main-glass pixels, opposing cyan/magenta traffic, body and fixed bezels are unchanged from needs-input-v2. Only the rear-screen contents and thirteen formerly paused fingertip pixels differ in the PNG frames.

The [review pack](review/pet.json) retains all six approved physical clips from compaction-v2 and replaces the needs-input candidate. Existing approved source frames, sprites, GIFs, manifests and motion metadata are copied exactly. The owner approved this refinement on 2026-10-05. The subsequent [interruption pack](../interrupted-v2/README.md) preserves these exports exactly.

## Authoring and inspection

[authoring.json](authoring.json) sets `attention.pause_typing` to `false` and `attention.symbol` to `warning_triangle`. The optional offline [compositor](../../../../examples/compaction_revision.rs) retains the old exclamation mark as the default, so earlier plans reproduce their original artwork. The new code-native glyph fits in a 9x7 area inside the 11x7 rear display, with 36 amber pixels and three dark punctuation pixels. No new character or vehicle raster artwork was needed.

[Native scene contacts](review/needs-input-contact.png), [rear-screen contacts](review/needs-input-rear-contact.png) and the [poster](review/needs-input-poster.png) were inspected. [Independent verification](review/verification.json) confirms all thirteen typing pixels match their corresponding approved working frame, with twelve distinct typing patterns preserved in the GIF too. All 4,006 pixels outside the rear display and fingertips match needs-input-v2 exactly. The visor housing, screen bezels, dark punctuation and triangle silhouette stay fixed; the triangle has highlight motion in addition to the unchanged pulse.

The new GIF uses one palette fitted across the complete loop. PNG device sprites retain full RGB, and previous approved GIFs remain byte-identical. GIF timing distributes 80/90 ms ticks for 2.650 seconds; the PNG manifest retains uniform 83 ms timing.

## Reproduce and check

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example compaction_revision -- pets/decker/revisions/needs-input-v3/authoring.json local/needs-input-v3-rebuild
python pets/decker/revisions/needs-input-v3/verify.py local/needs-input-v3-rebuild
```

Compilation, export and independent image checks passed. Rebuilding the previous needs-input-v2 plan after the optional symbol change reproduced all 224 native PNG frames and fourteen individual GIFs exactly, together with other PNG/GIF exports, the manifest and motion metadata.

Rust formatting, Clippy with warnings denied and bridge CLI validation were attempted but rejected by this sandbox's access-denied directory metadata checks. They are not marked passed. The existing external helper can complete them in your PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example compaction_revision -Pack pets/decker/revisions/needs-input-v3/review -Format
```

The helper checks the offline example and validates the pack. It does not upload artwork or change bridge settings. Delegation and interruption remain for later artwork revisions.
