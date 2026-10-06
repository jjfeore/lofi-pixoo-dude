# Decker work revision 7

This is a superseded forearm experiment. The owner initially called the thin segment the forearm, then clarified that they meant the upper arm. [Work-v8](../work-v8/README.md) starts from work-v6 and corrects the upper sleeve while restoring the original forearm. This revision is retained for comparison.

Work-v7 changes only the foreground forearm texture and its thickness normalization. The hand, upper sleeve, shoulder/elbow/wrist paths, visor timing, clean plate, far-hand retention and background timelines are preserved. Entry, finish and interruption all use the thicker forearm. Accepted steady working PNG frames and native/enlarged GIFs remain exactly identical; idle-v3 RGB is unchanged.

View the [complete cycle](review/work-cycle-preview.gif), [entering work](review/working-enter-preview.gif), [finishing](review/finished-preview.gif), or [interruption](review/interrupted-preview.gif). The [entry contact sheet](review/working-enter-contact.png) includes the resting frame and first moving poses for comparison. All runtime clips retain 32 native 64x64 frames at 83 ms.

Built-in image generation edited the existing sleeve using the approved resting base as a proportion reference. The [source](sources/fore-sleeve.png) and exact [prompt](sources/prompts.md) are saved. The selected native forearm is 19x16 instead of 19x9; its main body has 13-16 opaque pixels per column instead of 8-9. The source is normalized once and rotates at unit scale throughout the existing gesture. The cuff remains substantial. Other generated assets are referenced from work-v6 rather than regenerated.

[authoring.json](authoring.json) records the source and thickness. Reproduce into a new directory from the repository root:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v7/authoring.json local/work-v7-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v7-rebuild
```

Optional independent read-only image analysis uses Python and Pillow:

```powershell
python pets/decker/revisions/work-v7/verify.py local/work-v7-rebuild
```

The saved [verification](review/verification.json) confirms the wider actual sleeve silhouette, identical motion records and hand images, unchanged other source exports, fixed limb lengths/unit scale, vacated near keyboard hand while lifted, stationary unoccluded far limb, native sprite ordering and unchanged accepted working RGB/GIF bytes. The pack validator accepts all five clips (2,621,440 prepared bytes). Rest/departure, full gesture contacts and raised-arm posters were visually inspected; owner review remains pending.

Individual GIFs use 80/90 ms ticks totaling 2.650 seconds; native playback retains 83 ms timing and full RGB. The combined desktop preview lasts 13.280 seconds. This is a separate review pack. No Rust source, running bridge, active config, device content or commit changed in this revision.
