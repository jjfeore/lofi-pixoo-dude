# Compaction with idle and working poses

Both compaction variants now use the approved decker and 32 native 64x64 frames at 83 ms (approximately 12 FPS, 2.656 seconds). Idle keeps the visor raised and the quieter main monitor; working keeps the chunky powered visor down, faster typing and brighter main monitor. All 209 pixels of the main monitor's angled glass animate. Both screen bezels stay fixed.

Preview [idle compaction](review/compacting-idle-preview.gif), [working compaction](review/compacting-working-preview.gif), or [both in sequence](review/compaction-cycle-preview.gif). The [review pack](review/pet.json) includes these variants plus the four approved idle, working, entering and finishing clips. Those four PNG/GIF/sprite exports are copied exactly from the accepted idle-v4 combined pack. No active bridge configuration or display content was changed.

## Motion

The rear display shows three data rows that gather into a 3x3 packet, pause, slide to the right and refill from the left. The first and last rear-screen pixels match, avoiding a reset at the loop boundary. There are eighteen distinct rear-display patterns across the full sequence. The packet is a small code-native geometric graphic; no character or scene raster regeneration was needed.

Idle uses quieter cyan/violet colors, a normal cyan car and a smaller distant car on two opposing single-pass lanes. Working uses brighter cyan/magenta and three vehicle types across three lanes, with five total passes. Phase offsets vary arrivals. The existing traffic masks keep vehicles within the window and preserve foreground silhouettes. Vehicle images are the existing approved raster assets.

[authoring.json](authoring.json) controls source references, traffic lanes/directions/passes/phases and rear-display colors. `examples/compaction_revision.rs` starts from each approved idle/working frame, changes only the rear screen and sky traffic, and preserves all other pixels exactly. It is an offline artwork tool with no network or bridge-runtime changes. Native [idle contacts](review/compacting-idle-contact.png), [working contacts](review/compacting-working-contact.png), [idle rear crops](review/compacting-idle-rear-contact.png) and [working rear crops](review/compacting-working-rear-contact.png) support visual inspection. The owner approved both variants on 2026-10-05. The subsequent [needs-input pack](../needs-input-v2/README.md) preserves their exports exactly.

## Reproduce and verify

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example compaction_revision -- pets/decker/revisions/compaction-v2/authoring.json local/compaction-v2-rebuild
python pets/decker/revisions/compaction-v2/verify.py local/compaction-v2-rebuild
```

The offline example compiled and exported successfully. [Independent saved-image verification](review/verification.json) passed all 3,678 pixels outside the overlays against the corresponding approved frame, exact main-screen/fingertip pixels, 209 animated glass pixels, 214 fixed bezel-region pixels, the compression phases and 3x3 packet, a matching rear-screen loop join, visible traffic on each lane, native sprite ordering, shared GIF palette and native/enlarged GIF samples. Both clips have 32 distinct full frames. Working traffic has greater measured visible coverage as well as more passes. The four base clips, motorized motion metadata and previous work-cycle preview remain byte-identical.

The manifest's `compacting` selection maps aggregate `idle` and `working` poses to their full-scene variants. Its default source is the idle compaction sprite, so every manifest selection references a complete 32-frame source. The runtime already supports this variant contract; no new hook or state handling was added.

GIF delays distribute 80/90 ms ticks for 2.650 seconds per individual loop. The two-variant desktop preview has 64 frames and 5.310 seconds; it is not a device clip. Native PNG playback retains uniform 83 ms timing.

## Remaining tool checks

The sandbox rejected Rust formatting with `Access is denied`, Clippy with `failed to read CARGO_MANIFEST_DIR as a directory: Access is denied`, and bridge validation with `pack directory not found: Access is denied`. These checks are not marked passed. Run the [external helper](../../../../scripts/check-artwork.ps1) in your PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example compaction_revision -Pack pets/decker/revisions/compaction-v2/review -Format
```

`-Format` formats only the selected offline example before Clippy; the helper then validates the pack. It does not upload, change device settings or rebuild the bridge runtime. Its PowerShell syntax was checked. Delegation and interruption artwork remain for subsequent revisions.
