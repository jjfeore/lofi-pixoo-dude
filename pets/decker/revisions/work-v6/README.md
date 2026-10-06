# Decker work revision 6

This review pack switches the visor gesture to the foreground arm. The owner found work-v5's moving background arm too thin and requested the more visible near arm as the proportion reference. Entry, finish and interruption now lift the near hand; the far arm and keyboard hand stay in place. The accepted work-v4 steady working PNG frames and both GIF exports remain exactly identical. Approved idle-v3 RGB is unchanged.

Owner review found this version almost right and later clarified that the upper arm was too thin. [Work-v8](../work-v8/README.md) preserves this motion and forearm while broadening the upper sleeve. Work-v7 expanded the forearm in response to the initial wording and is retained as a superseded experiment. This version is the motion/source baseline.

View the [complete cycle](review/work-cycle-preview.gif): entry, two working loops, finish, idle. Individual previews: [entering work](review/working-enter-preview.gif), [working](review/working-preview.gif), [finishing](review/finished-preview.gif), [interruption](review/interrupted-preview.gif). Native [entry contacts](review/working-enter-contact.png) and [finish contacts](review/finished-contact.png) show every frame.

All five runtime clips retain 32 native 64x64 frames at 83 ms, lasting 2.656 seconds. Entry, finish and interruption are non-looping in the manifest; their individual GIFs repeat for review. Working links to entry. The rear monitor and varied traffic continue through the gestures, and the whole main-monitor glass keeps its accepted animation.

## Foreground gesture and exposed scene

The broader foreground jacket and lower typing hand provide the reference. Generated pieces are normalized once to a 16x11 upper sleeve, 19x9 fore sleeve and 11x9 gripping hand. The fore sleeve's central columns have eight or nine opaque pixels, with a substantial cuff, instead of narrowing into a thin tube. The new hand contains 48-56 visible skin pixels across the gesture, compared with 48 in the original near typing hand. These measurements help check proportions; owner visual review remains necessary.

The shoulder stays fixed, with 15- and 18-pixel bone lengths and unit-scale rotations. The elbow bends along a consistent branch, and the wrist follows a reachable path to the visor. The foreground layer draws in front of the body and can naturally occlude the face or rear monitor. It is no longer clipped behind the stationary near hand as previous far-arm gestures were.

A generated clean plate removes the original foreground sleeve, its violet band and lower keyboard hand before drawing the lifted limb. It exposes the body, far sleeve and keyboard underneath. The original visible far arm and hand pixels are preserved through configured retained regions; neither hand types during the gesture. The far limb is fixed wherever the moving foreground layer does not occlude it. No extra near hand remains on the keyboard while raised.

## Sources and reproduction

Built-in image generation supplied the foreground sleeve pieces, gripping hand and clean plate. The [saved prompts](sources/prompts.md) include refinements that removed a remaining old sleeve band and prevented the fore cuff from tapering. Selected sources are [upper sleeve](sources/upper-sleeve.png), [fore sleeve](sources/fore-sleeve-v2.png), [hand](sources/grip-hand.png) and [clean plate](sources/near-arm-clean-v2.png). Initial source versions are retained. The clean plate is normalized mechanically to [64x64](sources/near-arm-clean-v2-64.png); only configured removal regions are sampled into runtime frames.

[authoring.json](authoring.json) records the foreground shoulder, wrist paths, texture dimensions, hand rotations, removal rows and retained far-limb regions. `foreground: true` allows the near layer to draw through the bottom of the canvas and pins both keyboard fingertip poses during its gesture. `fore_width` separates fore and upper texture thickness. Absent fields retain earlier authoring behavior. The unused legacy whole-arm source fields are retained for compatibility. The accepted working GIF palette is reused to prevent color changes in the preserved working loop.

The offline Rust authoring helper has no device or network access. From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v6/authoring.json local/work-v6-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v6-rebuild
```

Optional independent read-only PNG/GIF analysis uses Python and Pillow:

```powershell
python pets/decker/revisions/work-v6/verify.py local/work-v6-rebuild
```

## Checks and review

The saved [verification](review/verification.json) checks fixed bone lengths and unit scale in all 29 articulated frames of each gesture, substantial actual sleeve alpha coverage, original foreground-hand removal in lifted poses, unchanged unoccluded far-arm/hand pixels, and 2,387-2,637 unchanged unobstructed scene pixels per gesture frame. It also verifies native/sprite ordering, background movement, full main-screen animation, exact accepted working RGB/GIF bytes, unchanged idle RGB, canonical endpoints and GIF timing. The [motion record](review/rig-motion.json) and `review/rig-frames/` preserve per-frame pose and limb evidence. Native contacts and enlarged gesture posters were inspected; these checks are not an automatic anatomy assessment.

The bridge validator accepts five clips (2,621,440 prepared bytes). The example passes Clippy with warnings denied. All 160 prior work-v5 RGB frames reproduce identically after these optional authoring changes.

Native PNGs retain full RGB. Individual GIFs use 80/90 ms ticks totaling 2.650 seconds; native playback uses 83 ms. The 160-frame combined GIF is a 13.280-second desktop preview. Canonical endpoints match frame zero; real hook changes and device uploads still have approximate timing. This remains a separate review pack with owner review pending. No bridge-runtime change, device upload or active-pack switch accompanies this revision.
