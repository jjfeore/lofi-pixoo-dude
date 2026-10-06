# Contact and visor travel revision

Superseded by [work-v12](../work-v12/README.md). Owner review found the hand changing depth at the keyboard joins and a wrist curling around from behind the head. Lens-spacing checks passed but did not establish correct foreground anatomy; v12 rebuilds the action through progressive whole-character studies.

Owner review of work-v10 found that the hand stopped short of the parked visor and the visor skipped directly to the eyes. This revision adds complete-character grip and intermediate travel poses, preserving the broad foreground sleeve and purple band. Both directions explicitly reach the current visor position, establish contact, carry it to its destination, release, and return the hand to the keyboard.

View [starting work](review/working-enter-preview.gif), [finishing](review/finished-preview.gif), or the [complete cycle](review/work-cycle-preview.gif). Both actions have 32 native 64x64 frames at 83 ms (2.656 seconds). The accepted idle-v3 and steady working-v4 PNG frames and both GIF exports remain exact. Traffic, the full main-glass animation and rear-screen patterns continue forward through both actions; the rear screen returns to idle during finish.

## Gesture timeline

Frame numbers are zero-based. Finish reverses the character action while the ambient compositor keeps moving forward.

| Phase | Start frames | Finish frames |
| --- | --- | --- |
| Reach from keyboard | 1-9, toward forehead | 1-9, toward eye visor |
| Establish grip | 10-11, forehead hold | 10, eye contact |
| Carry held visor | 12-20, lower gradually | 11-19, raise gradually |
| Destination contact | 21, eye contact | 20-21, forehead hold |
| Release and return hand | 22-30 | 22-30 |
| Canonical endpoint | 31, accepted working frame 0 | 31, accepted idle frame 0 |

Native [start contacts](review/working-enter-contact.png), [finish contacts](review/finished-contact.png), [start head crops](review/working-enter-heads.png) and [finish head crops](review/finished-heads.png) show every exported frame. They require visual review of hand contact, clothing, identity and occlusion; counting unique full-scene frames is insufficient because moving cars can make a held or incomplete gesture unique.

## Sources and registration

The built-in image generation tool supplied [eight travel poses](sources/visor-travel.png), a [forehead grip edit](sources/forehead-contact.png), [two middle poses](sources/middle-inbetweens.png) and a [small lowering step](sources/small-step.png). These supplement the existing work-v10 complete-character atlas and corrected forehead source. They retain whole-character anatomy rather than constructing a separate arm skeleton.

Exact prompts are saved for the [travel atlas](sources/prompts.md), [forehead contact](sources/single-contact-prompt.md), [middle pair](sources/single-inbetweens-prompt.md) and [small step](sources/small-step-prompt.md). A broad [atlas edit](sources/travel-edit-prompt.md) did not fully fix the contact and jump; a [four-pose study](sources/contact-inbetweens-prompt.md) changed framing. Both unused attempts remain under sources for comparison and are excluded from the final timeline. Generated poses do not necessarily obey requested pixel coordinates, so their actual native positions are measured after export.

`examples/atlas_cells.rs` mechanically extracts original-resolution RGBA cells for focused edits. The saved [original cells](sources/travel-original-cells/) retain exact source pixels and alpha. For example:

```powershell
cargo run --locked --release --example atlas_cells -- pets/decker/revisions/work-v11/sources/visor-travel.png local/travel-original-cells 4 8
```

[authoring.json](authoring.json) selects 30 distinct source poses across 32 frames, with two short holds. The source library has 44 complete-character poses; this does not increase the device clip length. The original atlas retains its 48x56 projection at `[0,8]`. Supplementary atlases use a common 48x54 projection at `[2,10]`, aligning their generated framing with the original head and body. Each source is registered once, without fitting individual limbs or dynamically changing scale during the gesture. The existing reconstructed room supplies newly revealed backdrop; both limbs are drawn in each complete-character pose. Only one visor is illuminated at its current position, including through the reversed finish.

## Reproduce and verify

From the repository root, export into a new directory:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v11/authoring.json local/work-v11-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v11-rebuild
```

Optional independent read-only PNG/GIF analysis uses Python and Pillow; it writes only a verification JSON record:

```powershell
python pets/decker/revisions/work-v11/verify.py local/work-v11-rebuild
```

[Saved evidence](review/verification.json) checks actual lens travel, accepted idle/working RGB and GIF bytes, exact canonical endpoints, 946 fixed scene pixels, the main bezel, all 209 animated working-glass pixels, absent duplicate forehead glow after lowering, room reconstruction, hand/band pixel coverage, original RGBA cell extraction, sprite order and GIF timing. Both transitions have 32 unique full-scene frames. The largest positive vertical lens-centroid step is 1.75 native pixels through the held travel, compared with 8.25 in work-v10. Lens centroids establish spacing, not anatomical contact or hand count. Native contacts and gesture posters were inspected; owner review remains pending. Small generated face/cloth differences can remain.

Bridge validation accepts four clips and 2,097,152 prepared bytes. Both offline examples pass Clippy with warnings denied. All 128 previous work-v10 RGB frames and all eight individual GIF files reproduce identically after the optional atlas additions. GIF previews use 80/90 ms ticks totaling 2.650 seconds per clip; PNG playback retains 83 ms per frame. The combined desktop cycle has 160 frames and lasts 13.280 seconds.

This is a separate review pack. Other lifecycle art awaits its own revision. No bridge runtime source, release bridge executable, active configuration, device upload or commit accompanies this artwork update.
