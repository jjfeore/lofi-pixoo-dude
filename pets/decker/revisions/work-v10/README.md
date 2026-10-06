# Whole-character start and finish animations

Owner review found missing hand-to-visor contact and intermediate visor travel in both directions. This candidate is retained for comparison; [work-v11](../work-v11/README.md) adds explicit contact and held-travel poses.

The owner approved the four-pose study and requested start/finish animations based on it. This revision replaces the separate limb rig with generated complete-character poses: torso, actual shoulder attachment, bulky sleeve, violet band, both arms and both hands are drawn together. The rear limb is reconstructed where the near arm reveals it. Accepted idle-v3 and steady working-v4 remain exactly unchanged in native RGB and both GIF exports.

View [starting work](review/working-enter-preview.gif), [finishing](review/finished-preview.gif), or the [complete cycle](review/work-cycle-preview.gif). Each transition has 32 native 64x64 frames at 83 ms, lasting 2.656 seconds. Entry lowers the visor with the foreground hand and returns that hand to the keyboard. Finish reverses the character action; traffic and rear-screen motion continue forward and return to the idle pattern. Native [entry contacts](review/working-enter-contact.png) and [finish contacts](review/finished-contact.png) show every frame.

## Sources and compositing

Built-in image generation produced a transparent 32-pose [character atlas](sources/character-atlas.png), a reconstructed [room plate](sources/room-clean.png), and a targeted [forehead edit](sources/character-visor-clean.png). Exact [generation prompts](sources/prompts.md) and [visor cleanup prompt](sources/visor-cleanup-prompt.md) are saved. The original generated sequence had a few poses out of chronological order; [authoring.json](authoring.json) selects 27 source poses with five brief holds across the 32-frame timeline. Every final start and finish frame is unique because ambient motion continues. These are complete character key poses, not independent arm parts fitted to changing hand destinations.

The atlas's complete scene cells use one common 48x56 projection at `[0,8]` inside the 64x64 viewport. They are never cropped to separate per-pose bounds or rescaled during the gesture. Alpha is thresholded for hard pixels and colors are registered to the approved scene palette. The room plate supplies only configured regions formerly occupied by the original character; room pixels outside them retain the approved base. Monitors stay in place and retain the whole-glass projection. The character layer can naturally occlude the rear monitor.

During generation, the owner noticed that the lowered visor left a second glow on the forehead. The generated edit is sampled only in the forehead region for lowered poses, preserving the original atlas elsewhere. Two corrected source cells still had stray cyan pixels; their forehead regions use neighboring correctly dark generated cells. All 19 lowered source poses now have zero active cyan pixels in the checked forehead region. The active face visor remains illuminated. Finishing restores the raised visor as the character action reverses.

## Reproduction and checks

The optional full-character path lives in the offline Rust authoring example and has no device or network access. Reproduce into a new directory from the repository root:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v10/authoring.json local/work-v10-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v10-rebuild
```

Optional independent read-only analysis uses Python and Pillow:

```powershell
python pets/decker/revisions/work-v10/verify.py local/work-v10-rebuild
```

The saved [verification](review/verification.json) checks exact accepted idle/working RGB and GIF bytes, canonical transition endpoints, 946 fixed native scene pixels, the unchanged main bezel, all 209 animated working-glass pixels, sheet ordering, GIF timing, changing rear-screen and traffic patterns, original skin removal from the reconstructed room, keyboard-hand and violet-band coverage, and the absent duplicate forehead glow. Generated full-character [pose exports](review/character-poses/) and [timeline metadata](review/character-motion.json) are preserved. Coverage checks are not an automatic anatomy or hand-count assessment. Native full-scene/head contacts and enlarged gesture posters were inspected; owner review remains pending. Small generated face/cloth differences may still be visible.

Bridge validation accepts four clips and 2,097,152 prepared bytes. The example passes Clippy with warnings denied. All 160 work-v8 native RGB frames reproduce identically after adding the optional full-character path. Individual GIFs use 80/90 ms ticks totaling 2.650 seconds; native playback uses 83 ms. The combined 160-frame desktop cycle lasts 13.280 seconds.

This is a separate review pack for start/finish. Other lifecycle artwork remains at its previous baseline. No bridge runtime source, release bridge executable, active configuration, device content or commit changed.
