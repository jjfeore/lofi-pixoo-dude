# Idle: full monitor glass

This revision fixes the small animated inset on the idle main monitor. Its visible screen is nine angled columns totaling 209 pixels, matching the accepted working-screen outline. The original contents scroll within each column's glass boundaries; the bezel stays fixed. Each column makes one full cycle over 32 frames at 83 ms. The loop boundary advances one source row, and the first frame is identical to the approved idle-v3 first frame.

View the [idle GIF](review/idle-preview.gif), [full contact sheet](review/contact.png), or [monitor crops](review/monitor-contact.png). The [idle-only pack](review/pet.json) can be selected independently. The [combined pack](combined/pet.json) includes the accepted motorized start/finish and steady working clips with the new idle screen. Its [cycle preview](combined/work-cycle-preview.gif) shows the four selections together.

All 3,887 pixels outside the main glass match idle-v3 in every corresponding native frame. That includes the hands, face, visor, jacket, monitor housing, traffic and rear oscilloscope animation. The existing idle GIF palette is reused, preserving decoded preview colors outside the glass too. All 209 glass pixels change over the loop. This changes the existing editable compositor and reuses the approved raster assets; no new character or scene generation was needed.

## Export and verify

From the repository root, choose new output directories:

```powershell
cargo run --locked --release --example idle_revision -- pets/decker/revisions/idle-v4/authoring.json local/idle-v4-rebuild
python pets/decker/revisions/idle-v4/verify.py local/idle-v4-rebuild
```

[authoring.json](authoring.json) holds the full-glass outline, accepted typing schedule, traffic paths and waveform reference. Optional `monitor_aperture` and `palette_from` fields leave older plans unchanged. [work-authoring.json](work-authoring.json) rebuilds the combined pack with `work_revision`; it references `review/frames` from this revision. Build that idle directory first if recreating the combined pack from scratch.

The offline idle example compiled and both packs exported successfully. [Read-only verification](review/verification.json) passed native frame/sprite ordering, 32-frame timing, all 209 animated glass pixels, 115 fixed bezel-region pixels, identical pixels outside glass, preserved original column colors, an unchanged first frame, one-row loop wrap and exact native/enlarged GIF samples. The idle-only GIF has 80/90 ms ticks totaling 2.650 seconds; PNG playback retains 83 ms and 2.656 seconds. The combined cycle has 160 frames and 13.280 seconds.

Combined start/finish change only main-glass pixels relative to work-v13. Their motorized motion metadata, typing, rear patterns, traffic and canonical endpoints remain unchanged. Steady working RGB and both GIF files are exact. Regression reproduced all 32 original idle-v3 RGB frames and both GIF files exactly with the updated helper.

## Remaining tool checks

This sandbox rejects directory access for Rust formatting, Clippy (`failed to read CARGO_MANIFEST_DIR as a directory: Access is denied`) and the bridge pack validator (`pack directory not found: Access is denied`). These checks are not recorded as passed. Run the [check helper](../../../../scripts/check-artwork.ps1) in your own PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example idle_revision -Pack pets/decker/revisions/idle-v4/review,pets/decker/revisions/idle-v4/combined -Format
```

`-Format` formats only the named offline example before Clippy. The script validates both packs and does not rebuild the bridge runtime, upload to a display or edit device settings. Its PowerShell syntax was checked. On 2026-10-05 the owner approved the updated idle animation. The subsequent [compaction pack](../compaction-v2/README.md) preserves its exports exactly; the running bridge and physical display have not been switched to these review packs.
