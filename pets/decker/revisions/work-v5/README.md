# Decker work revision 5

This review pack revises the hand-assisted entry, finish and interruption gestures. The owner accepted work-v4's steady working loop, full monitor animation and chunky visor, but found that the moving arm appeared to shrink into the background. The accepted working PNG frames and both working GIF exports are preserved exactly, including their palette. Approved idle-v3 RGB frames are unchanged.

Owner review found this version's moving background arm too thin and requested the foreground arm for visor operation. [Work-v6](../work-v6/README.md) supersedes these gestures with broader foreground pieces while keeping the far keyboard limb fixed. Work-v5 remains a retained source/reproduction baseline.

View the [complete cycle](review/work-cycle-preview.gif): entering work, two working loops, finishing, idle. Individual previews: [entering work](review/working-enter-preview.gif), [working](review/working-preview.gif), [finishing](review/finished-preview.gif), and [interrupted](review/interrupted-preview.gif).

All five runtime clips use 32 native 64x64 frames at 83 ms, lasting 2.656 seconds. Entering, finishing and interruption are non-looping in the manifest; their standalone GIFs repeat for review. Working links to the entry clip. These exports are a separate review pack; the bridge runtime and active display configuration are unchanged.

## Arm motion

Work-v4 fitted each complete generated arm pose to its shoulder and hand destinations. Its entry transforms changed scale from approximately 0.323 to 0.813, which made the limb appear to recede. This version uses three generated transparent pieces: an upper sleeve, fore sleeve and compact gripping hand. Each is cropped and resized once to native dimensions, then rotated and translated without scaling.

The shoulder stays fixed. The upper arm remains 12 pixels long, the forearm 15 pixels, and the hand uses the same 7x7 texture throughout the gesture. A two-bone joint calculation bends the elbow to reach the configured wrist path; unreachable poses are rejected rather than stretching the arm. The same elbow branch is used throughout. The wrist rotates separately, contacts the visor, follows its movement, releases it and returns to the keyboard. Pixel coverage can vary slightly during nearest-neighbor rotation, while geometric size stays fixed.

The original far forearm and keyboard hand are cleared before drawing the moving limb. Far-hand typing is disabled during the gesture. The nearer hand remains on the keyboard with its approved fingertip motion; the moving limb stays behind it. Interruption uses the same revised raising motion with work-v4's restrained warning pulse.

Native [entry contacts](review/working-enter-contact.png), [finish contacts](review/finished-contact.png) and [interruption contacts](review/interrupted-contact.png) show every frame. `review/rig-frames/` contains the composite limb and unoccluded hand for each articulated frame. The saved [motion record](review/rig-motion.json) records the shoulder, elbow, wrist, bone lengths and transform scale in every pose.

## Preserved scene and background motion

The chunky 11x6 powered visor and all 209 pixels of the main monitor's angled glass retain work-v4's appearance and animation. The housing stays fixed. The rear monitor changes between the green oscilloscope and the magenta/cyan working pattern. Varied traffic keeps moving during each gesture. Working and idle artwork is not regenerated.

## Sources, reproduction and checks

Built-in image generation supplied the [upper sleeve](sources/upper-sleeve.png), [fore sleeve](sources/fore-sleeve.png) and [grip hand](sources/grip-hand.png); exact [prompts](sources/prompts.md) are saved. The clean plate and chunky visor come from work-v4, screen/forehead/traffic sources from work-v3, and the base and idle sources from idle-v3.

[authoring.json](authoring.json) records the texture sizes, fixed limb lengths, wrist paths, hand rotations, clean-plate masks and existing background timelines. In articulated mode, the legacy `enter_hands` and `exit_hands` fields specify wrist positions; the legacy whole-arm source is retained for compatibility but not loaded. `palette_from` locks GIF previews to the accepted working palette. The offline compositor in `examples/work_revision.rs` has no network or device access and is not part of bridge playback.

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v5/authoring.json local/work-v5-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v5-rebuild
```

Optional independent read-only PNG/GIF analysis uses Python and Pillow:

```powershell
python pets/decker/revisions/work-v5/verify.py local/work-v5-rebuild
```

The [verification record](review/verification.json) confirms fixed bone lengths and unit scale in all 29 articulated frames of each gesture, no wrist/elbow jumps above 5.1 pixels per frame, vacated keyboard-hand skin while lifted, preserved nearer hand, native sprite ordering, 32 frames per clip, background motion, full main-screen animation, exact accepted working RGB/GIF bytes and unchanged idle RGB. It also records unoccluded hand coverage so projection-induced shrinking can be distinguished from small rasterization changes. Native contacts and enlarged gesture/head previews were inspected; these checks do not automatically establish anatomical quality. Owner review remains pending.

The bridge validator accepts five clips (2,621,440 prepared bytes). The authoring example passes Clippy with warnings denied. All 160 prior work-v4 RGB frames reproduce identically after the optional rig and palette additions.

Native PNGs retain full RGB. Individual GIF delays use 80/90 ms ticks totaling 2.650 seconds; native playback uses constant 83 ms. The 160-frame complete-cycle GIF is a desktop preview lasting 13.280 seconds. Canonical endpoints match frame zero; live hook changes and device uploads still have approximate timing. No device upload or active pack change accompanies this artwork revision.
