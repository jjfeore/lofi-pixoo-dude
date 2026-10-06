# Decker work revision 4

This review pack revises the visor and hand gestures from work-v3. It also expands the working main-monitor animation to its full visible angled screen. The approved idle-v3 base and native idle frames are retained.

Owner review accepted the steady working loop, full monitor appearance and chunky visor, but rejected the apparent shrinking of the gesture arm. [Work-v5](../work-v5/README.md) preserves this working loop exactly and replaces only the gesture limb with fixed-size pieces rotating at the joints. This version is retained as its source and reproduction baseline.

View the [complete cycle](review/work-cycle-preview.gif): entering work, two working loops, finishing, idle. Individual previews: [entering work](review/working-enter-preview.gif), [working](review/working-preview.gif), [finishing](review/finished-preview.gif), and [interrupted](review/interrupted-preview.gif).

All five runtime clips use 32 native 64x64 frames at 83 ms, lasting 2.656 seconds. Entering, finishing and interruption are non-looping in the manifest; their standalone GIFs repeat for review. Working links to the entry clip.

## Visor and hands

The lowered visor has an 11x6-pixel chunky housing, thick violet/navy outline and contained cyan lens, matching the older working style. The approved raised idle goggles remain the start/end pose.

One far arm lifts from the keyboard, grasps the visor, lowers or raises it, releases it and returns. The nearer hand stays on the keyboard with the approved fingertip motion. Interruption includes a restrained red warning pulse followed by the same hand-assisted raising gesture.

The generated transparent 4x4 arm atlas contains sixteen key poses. Their shoulder and hand landmarks are registered to this exact base using nearest-neighbor similarity transforms, with the shoulder held at the configured coordinate and the hand aligned to each gesture target. The 32-frame timeline moves through those poses; it does not repeat an eight-frame action.

Before an arm pose is composited, a generated clean plate replaces the original far forearm and keyboard hand. Far-hand typing pixels are disabled during the gesture. The animated arm is behind the nearer foreground hand, so it cannot add another hand on top of that hand. Only one arm layer is drawn per frame.

The independent export check verifies the vacated keyboard-hand skin pixels throughout the lifted part of each gesture and preserves the nearer hand outside its previously approved fingertip pixels. Native contacts and enlarged gesture posters were inspected. Those checks support the specific duplicate-hand fix; they are not a general automatic anatomy classifier.

## Full main screen and background motion

The previous working insert covered an 80-pixel rectangle. This version maps the source over all 209 visible glass pixels in nine columns, each with its own angled top and bottom edge. Every glass pixel changes across the working loop. The monitor bezel stays identical to the base in all frames, including interruption. Working contents scroll through the whole aperture; entry and finishing blend to/from the approved idle appearance.

The rear monitor changes from the green oscilloscope to a scrolling magenta/cyan processing pattern and returns when finished or interrupted. Traffic continues forward during every gesture, with varied lanes, cars, directions and pass frequency. Working also includes a smaller faster distant car. The arm can naturally occlude part of the rear monitor while reaching upward.

## Sources, reproduce and validation

Built-in image generation supplied the [chunky visor](sources/chunky-visor.png), [far-arm clean plate](sources/far-arm-clean.png) and [transparent arm atlas](sources/far-arm-poses.png). Exact [prompts](sources/prompts.md) are saved. Work-v3 supplies the clean forehead and screen/traffic sources; idle-v3 supplies the accepted base, fingertip motion, waveform and cyan car.

[authoring.json](authoring.json) records native regions, the full screen aperture, original-hand removal, shoulder/hand registration targets, gesture timing and background traffic. The offline compositor in `examples/work_revision.rs` performs mechanical transforms, compositing and export; it is not part of bridge playback and has no device access.

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v4/authoring.json local/work-v4-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v4-rebuild
```

Optional independent read-only image analysis uses Python and Pillow:

```powershell
python pets/decker/revisions/work-v4/verify.py local/work-v4-rebuild
```

The saved [verification record](review/verification.json) checks exact native/sprite dimensions and ordering, fixed scene pixels, main bezel and nearer-hand preservation, original-hand removal during lifted poses, all 209 animated screen pixels, background motion, unchanged approved idle RGB, canonical endpoints and shared GIF colors. The bridge validator accepts all five clips (2,621,440 prepared bytes). Clippy passes with warnings denied. After updating the helper, all 128 prior work-v3 RGB frames reproduced identically.

Native PNGs retain full RGB. Individual GIF delays are 80/90 ms ticks totaling 2.650 seconds; native playback uses constant 83 ms. The 160-frame complete-cycle GIF is only a desktop preview and lasts 13.280 seconds. Each runtime clip fits the 40-frame ceiling.

Canonical state endpoints match frame zero; live hooks can switch at another loop phase, and physical uploads still take time. Owner review accepted the working artwork; its gesture feedback is addressed by work-v5. The pack is portable at `review/`; no device upload, active-config change or bridge-runtime change was made for this artwork revision.
