# Progressive foreground-arm pose studies

Superseded by [work-v13](../work-v13/README.md). Owner review found the generated motion increasingly jittery and replaced the hand-operated gesture with an automatic motorized flip while the decker keeps typing. The studies below remain historical experiments.

Owner review of work-v11 found the moving hand passing behind the far arm at the typing joins and a wrist curling around from behind the head at the forehead grip. This revision follows the proposed hierarchy: establish complete-character anchors, generate separate interval studies, then subdivide remaining gaps. The moving foreground wrist connects to a visible cuff below the palm, and the forearm overlaps the near cheek during the grip.

View [starting work](review/working-enter-preview.gif), [finishing](review/finished-preview.gif), or the [complete cycle](review/work-cycle-preview.gif). Each action has 32 native 64x64 frames at 83 ms (2.656 seconds), selecting 30 distinct complete-character poses. Finish reverses only the character action. Traffic and both monitor timelines continue forward; the rear display returns to the idle waveform. Accepted idle-v3 and steady working-v4 RGB frames are exact, and their existing work-v11 review GIF exports are unchanged.

## Studies and sources

All creative raster work uses built-in imagegen with transparency. Original generated sources and exact prompts are saved locally; Rust handles cell extraction, registration, palette mapping, composition and export. No articulated arm sprites or changing per-limb scale are used.

| Study | Source | Prompt |
| --- | --- | --- |
| Five anchors plus a repeated first pose | [anchor sheet](sources/anchors-raw.png) | [anchor prompt](sources/anchors-prompt.md) |
| A-B: near hand leaves keyboard | [six intermediates](sources/interval-ab.png) | [prompt](sources/interval-ab-prompt.md) |
| B-C: foreground reach to visor | [six intermediates](sources/interval-bc.png) | [prompt](sources/interval-bc-prompt.md) |
| C-D: held visor lowering | [six intermediates](sources/interval-cd.png) | [prompt](sources/interval-cd-prompt.md) |
| D-E: release and return | [six intermediates](sources/interval-de.png) | [prompt](sources/interval-de-prompt.md) |
| Gap inside keyboard lift | [four poses](sources/lift-gap.png) | [prompt](sources/lift-gap-prompt.md) |
| Gap inside chest-to-keyboard return | [four poses](sources/return-gap.png) | [prompt](sources/return-gap-prompt.md) |
| First tiny visor tilt | [single pose](sources/visor-first-tilt.png) | [prompt](sources/visor-first-tilt-prompt.md) |
| Early visor travel | [two poses](sources/visor-early-gap.png) | [prompt](sources/visor-early-gap-prompt.md) |
| Lower-forehead visor position | [single pose](sources/visor-lower-forehead.png) | [prompt](sources/visor-lower-forehead-prompt.md) |

A is typing with the visor raised, B is the foreground hand at chest height, C grips the raised visor, D grips the lowered visor, and E returns to the same typing posture as A with the visor down. The [held anchor preview](studies/anchors/working-enter-preview.gif) is an anatomy study, not the final timing. Its [authoring plan](anchor-study.json) reproduces that study. Original-resolution RGBA cells are saved alongside each multi-cell source.

The first interval export still skipped parts of visor travel and the arm's return. Focused gap studies fill those movements. A [four-cell visor attempt](sources/visor-first-gap-attempt.png) mostly repeated endpoints and displaced the lens sideways; it is excluded from the final source library/timeline. Additional individual edits provide usable partial tilts. A requested number of generated cells does not guarantee the same number of useful motion poses.

## Final sequence and registration

Frame numbers are zero-based. The final [authoring.json](authoring.json) contains 42 source cells and selects 30 distinct poses across 32 frames, with brief holds.

| Phase | Starting work | Finishing |
| --- | --- | --- |
| Reach with foreground hand | 1-11 toward forehead | 1-10 toward eye visor |
| Contact at initial position | 12-13 on forehead | 11 at eyes |
| Carry the visor | 14-19 downward | 12-17 upward |
| Contact at destination | 20 at eyes | 18-19 on forehead |
| Release and return | 21-30 | 20-30 |
| Canonical endpoint | 31: working frame 0 | 31: idle frame 0 |

Anchors use a fixed 48x56 whole-cell projection at `[0,8]`. Interval and arm-gap sheets use 48x54 at `[1,10]`; focused visor sources use 48x54 at `[1,9]` to align their framing. Each atlas is registered once. No individual pose is fitted by its changing bounding box. The reconstructed room replaces the original character before each complete pose is composited; each pose supplies both limbs, including exposed far-arm regions.

[Start contacts](review/working-enter-contact.png), [finish contacts](review/finished-contact.png), [start head crops](review/working-enter-heads.png), [finish head crops](review/finished-heads.png) and the [eye-contact poster](review/eye-contact-preview.png) support frame-level review. Inspect depth, cuff/palm connection, two-hand anatomy and keyboard joins. Some generated face, clothing and resting-hand detail varies between studies; exact transition endpoints do not establish that every nearby join is visually seamless. Owner review of the new motion remains pending.

## Reproduce and verify

From the repository root, export into a new directory:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v12/authoring.json local/work-v12-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v12-rebuild
python pets/decker/revisions/work-v12/verify.py local/work-v12-rebuild
```

The optional Pillow analysis is read-only for image assets; it writes a JSON evidence record. [Saved verification](review/verification.json) confirms accepted RGB loops, unchanged previous review GIF bytes, canonical joins, 946 fixed scene pixels, the main bezel, all 209 animated working-glass pixels, continuing rear/traffic motion, exact extraction of 40 original RGBA cells, frame/sprite ordering and timing. The largest vertical lens-centroid step is about 1.93 native pixels over 8.33 pixels of held travel. Fully lowered selected poses have no saturated cyan forehead remnant; pale hardware highlights are not treated as an extra illuminated lens. Coverage and color measurements do not prove anatomical depth or hand count.

Bridge validation accepts four clips and 2,097,152 prepared bytes. Both new transitions have 32 unique full-scene frames. GIF ticks normalize to 80/90 ms, totaling 2.650 seconds; PNG playback retains 83 ms per frame. The combined desktop cycle has 160 frames and lasts 13.280 seconds. This revision changes artwork and authoring documentation only; no runtime code or activation is needed to review it.
