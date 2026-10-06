# Interrupted: jaw movement with the mouth opening

The [revised preview](review/interrupted-preview.gif) adds a small jaw drop and return to interrupted-v3. The chin lowers by one to two native pixels as the mouth opens, holds during the shout and rises as it closes. The cheek, upper face and rear jaw hinge stay steady. The three bold red rays, chase, screens, typing and automatic visor recovery are preserved.

The device clip remains forty 64x64 frames at 83 ms, totaling 3.320 seconds. The [context preview](review/interruption-cycle-preview.gif) includes working and idle for desktop review. Both device endpoints match the approved working/idle starts exactly.

[authoring.json](authoring.json) links jaw travel directly to the existing mouth stages. The offline [Rust compositor](../../../../examples/interruption_revision.rs) translates source chin/beard columns and their lower outline vertically, with less movement near the rear hinge. It extends the upper beard seam into the small exposed gap before drawing the mouth. No generated expression patch or horizontal jaw shift is used. A thirty-pixel region bounds the added movement; all 4,066 other native pixels match v3 in every frame.

## Reproduce and check

From the repository root, choose a fresh output directory:

```powershell
cargo run --locked --release --example interruption_revision -- pets/decker/revisions/interrupted-v4/authoring.json local/interrupted-v4-rebuild
python pets/decker/revisions/interrupted-v4/verify.py local/interrupted-v4-rebuild
```

Compilation, export and [independent PNG/GIF checks](review/verification.json) passed synchronized jaw travel, retained source chin/outline pixels, stable held shape, preserved v3 scene pixels, forty-frame timing, full-glass animation, fixed hands/bezel, chase, recovery and canonical endpoints. All seven prior physical clips remain exact: 224 frames and fourteen GIFs. Reproducing v3 also kept its PNG/GIF exports, manifests and motion records byte-identical, including 264 clip frames and sixteen individual GIFs. Native head, scene and chin/neck joins were inspected; owner review of v4 is pending.

Rustfmt, Clippy and bridge pack validation were attempted but blocked by sandbox directory-access errors. To complete them externally:

```powershell
.\scripts\check-artwork.ps1 -Example interruption_revision -Pack pets/decker/revisions/interrupted-v4/review -Format
```

This is a separate review pack; no runtime build, device upload, settings change or commit occurred.
