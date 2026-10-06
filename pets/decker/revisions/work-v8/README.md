# Decker work revision 8

The owner clarified that the thin segment was the upper arm, from shoulder to elbow. This revision starts from work-v6, before the large forearm expansion in work-v7. It broadens only the moving upper sleeve. The original forearm, hand, clean plate, wrist path, elbow motion and visor timing are retained. Entry, finishing and interruption share this correction. Accepted idle and steady working artwork remains unchanged.

Owner review still found insufficient upper-arm and band thickness, a low shoulder hinge and incomplete revealed rear-limb anatomy. The [whole-character pose study](../work-v9-pose-study/README.md) tests a different authoring approach; work-v8 remains a comparison candidate rather than an accepted transition.

View the [complete cycle](review/work-cycle-preview.gif), [entering work](review/working-enter-preview.gif), [finishing](review/finished-preview.gif), or [interruption](review/interrupted-preview.gif). The [entry contact sheet](review/working-enter-contact.png) includes the resting frame and first moving poses. Each runtime clip has 32 native 64x64 frames at 83 ms, lasting 2.656 seconds.

Built-in image generation edited the v6 upper sleeve using the approved base as a proportion reference. The selected [source](sources/upper-sleeve.png) and exact [prompt](sources/prompts.md) are saved. The normalized upper texture is 16x16 instead of 16x11; its main shaft contains 10-14 opaque pixels per column instead of 6-9. Every source column has greater opaque coverage. The original 19x9 forearm is restored exactly. Each piece is normalized once and rotates at unit scale, with the same 15/18-pixel bone lengths.

[authoring.json](authoring.json) records the source and thickness. Other source paths reuse v6 assets. Reproduce into a new directory from the repository root:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v8/authoring.json local/work-v8-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v8-rebuild
```

Optional independent read-only image analysis uses Python and Pillow:

```powershell
python pets/decker/revisions/work-v8/verify.py local/work-v8-rebuild
```

The saved [verification](review/verification.json) confirms wider native upper-arm coverage, identical v6 motion records and hand images, byte-identical other source exports including the original forearm, unchanged accepted working RGB/GIF bytes and idle RGB, fixed limb lengths, original-hand removal, stationary unoccluded background limb, full main-screen animation, native frame order and GIF timing. The bridge validator accepts five clips and 2,621,440 prepared bytes. Gesture contacts and raised-arm posters were visually inspected; owner review is pending.

Individual GIFs use 80/90 ms ticks totaling 2.650 seconds; native playback retains 83 ms timing and full RGB. The combined desktop preview lasts 13.280 seconds. This is a separate review pack. No Rust source, running bridge, active config, device content or commit changed in this revision.
