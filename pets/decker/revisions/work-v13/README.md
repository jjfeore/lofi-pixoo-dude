# Motorized visor, continuous typing

The owner replaced the hand-operated visor action after reviewing work-v12's jitter. The decker now continues typing while the visor automatically flips down for work and up for finish. This revision uses the approved character and existing visor layers directly in the editable offline compositor. No new character poses are generated. Face, jacket, arms, wrists and palms outside the visor/fingertip regions remain exact canonical pixels.

View [starting work](review/working-enter-preview.gif), [finishing](review/finished-preview.gif), or the [full cycle](review/work-cycle-preview.gif). Both clips have 32 native 64x64 frames at 83 ms (2.656 seconds). The accepted idle-v3/working-v4 RGB loops and existing work-v12 GIF loop exports are unchanged. Traffic and monitor patterns continue forward in both transitions; the background screen returns to the green idle waveform during finish.

## Motion

[authoring.json](authoring.json) uses the work-v3 forehead clean plate/screen patterns, work-v4 chunky visor, and idle-v3 fingertip/waveform/traffic assets. This is deterministic composition of the existing artwork; there are no new generation prompts or generated pose atlases.

The optional `motorized` settings control a smooth flip during frames 4-20 and screen-state change during frames 10-26. The visor's apparent height narrows to three pixels halfway through, then opens to the other endpoint, suggesting an edge-on motorized plate. Finishing inverts only visor/state progression. Both hands remain at the keyboard, with fingertip flexion sampled from the approved idle frames and faster cadence at work intensity. First/last images use exact accepted loop frame-zero endpoints.

[motorized-motion.json](review/motorized-motion.json) records every visor rectangle, state intensity and typing source frame. [Start contacts](review/working-enter-contact.png), [finish contacts](review/finished-contact.png) and [head crops](review/working-enter-heads.png) show the resulting native motion. On 2026-10-05 the owner accepted this as the current transition baseline. The subsequent [idle-v4 combined pack](../idle-v4/README.md) incorporates full-glass idle scrolling while preserving this visor motion.

## Reproduce and check

From the repository root, export into a new directory:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v13/authoring.json local/work-v13-rebuild
python pets/decker/revisions/work-v13/verify.py local/work-v13-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v13-rebuild
```

The offline Rust example compiled successfully. [Independent native image analysis](review/verification.json) passed: 3,248 fixed scene pixels, 2,105 fixed character/body pixels outside the permitted animation regions, all 208 hand/wrist/palm-region pixels outside the fingertips, accepted typing samples, canonical joins, unchanged loops, all 209 animated working-glass pixels, stable bezels/palette, no parked cyan glow after lowering, restored eye region after raising, sprite ordering and timing. These counts describe checked pixel regions rather than semantic anatomy segmentation. Typing, rear-display and traffic patterns change throughout the clips. GIF ticks use 80/90 ms and total 2.650 seconds; PNG playback retains 83 ms. The combined cycle is 160 frames and 13.280 seconds.

Regression of the shared helper reproduces all 128 previous work-v12 RGB frames and all eight individual GIF files exactly. The motorized mode is optional and does not alter older authoring plans.

**Remaining checks:** the current Codex sandbox rejects directory access for Clippy (`failed to read CARGO_MANIFEST_DIR as a directory: Access is denied`) and the executable's pack validator (`pack directory not found: Access is denied`). Relative, absolute extended paths and an isolated lint copy encountered the same restriction. Neither check passed in this sandbox; this does not invalidate the completed compilation/export/pixel analysis. Run [the check helper](../../../../scripts/check-motorized-artwork.ps1) in your own PowerShell:

```powershell
.\scripts\check-motorized-artwork.ps1
```

It checks the offline example and validates the review pack without uploading or changing bridge configuration. This revision changes only offline authoring and artwork; the bridge runtime executable remains untouched. Other lifecycle artwork still awaits its own revision.
