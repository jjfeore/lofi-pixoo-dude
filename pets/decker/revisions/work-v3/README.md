# Decker work revision 3

The owner requested a chunkier visor, hand-assisted entry/exit, and a full-screen working display after reviewing this version. [Work-v4](../work-v4/README.md) contains those revisions; this directory retains the previous candidate and its sources.

Review pack using the owner's approved native `base-v3.png` and the physically approved idle-v3 animation. The three new clips are `working-enter`, `working` and `finished`, each 32 native 64x64 frames at 83 ms per frame (2.656 seconds).

Open [the complete cycle](review/work-cycle-preview.gif) to see entering work, two working loops, finishing and an idle loop. Individual previews: [entering work](review/working-enter-preview.gif), [working](review/working-preview.gif), [finishing](review/finished-preview.gif). The individual GIFs repeat for review; the pack marks entering and finishing as non-looping.

Entering work lowers a hinged visor without moving the head or hands. The lens powers up, the main monitor becomes brighter and gains magenta diagnostics, and the rear monitor changes from the green oscilloscope to a scrolling magenta/cyan geometric processing pattern. Working keeps the visor down with a restrained power pulse, doubles the approved fingertip cadence and scrolls the main contents faster. Finishing raises the visor, softens the main screen and restores the green waveform.

Traffic continues forward through every clip. Entry has two opposing cars on the original lanes, including a new generated magenta vehicle. Working changes both lanes and adds a smaller, faster distant vehicle. Finishing changes direction/color combinations and the cyan car's pass frequency. Traffic is clipped behind the hair silhouette. Finishing reverses visor motion only, rather than reversing the whole scene.

The room, character identity, palms, wrists, keyboard and monitor housings retain the approved RGB pixels. A generated clean forehead plate is sampled only inside the raised visor's sparse footprint; the full generated scene is discarded. The separately generated transparent visor supplies its lowered pose. Screen inserts, a processing glyph and a magenta traffic sprite were also generated with the built-in image tool. Source images and exact prompts are in [sources](sources/prompts.md).

The offline Rust compositor applies mechanical masks, layer transforms, resizing and exports. It does not contact the Pixoo and is not part of the bridge's runtime. [authoring.json](authoring.json) records source paths, native regions, fingertip pixels, timing and per-clip traffic. These coordinates are specific to this base.

## Reproduce

From the repository root, with Rust and the locked dependencies available:

```powershell
cargo run --locked --release --example work_revision -- pets/decker/revisions/work-v3/authoring.json local/work-v3-rebuild
.\target\release\pixoo-pet.exe validate .\local\work-v3-rebuild
```

The output directory must not exist. On the development host, Cargo additionally used `--offline` and `CARGO_HOME=local/cargo-home`; that private cache is not required by the artwork or executable. The canonical 64x64 forehead export is saved with the sources; it is a nearest-neighbor mechanical reduction of the generated full scene.

Optional independent PNG/GIF verification uses Python with Pillow:

```powershell
python pets/decker/revisions/work-v3/verify.py local/work-v3-rebuild
```

The verifier reads artwork and writes a JSON evidence record. It checks native dimensions, sprite ordering, exact fixed pixels, pinned palms/bezels, approved idle preservation, canonical transition endpoints, changing traffic/monitor contents and shared GIF colors. [verification.json](review/verification.json) contains the recorded results. The Rust example passes Clippy with warnings denied, and the review pack passes the bridge validator.

All four runtime clips fit the 40-frame ceiling individually. The 160-frame combined GIF is only a desktop review preview. Its duration is 13.280 seconds; individual GIFs use 80/90 ms ticks totaling 2.650 seconds, while the native pack uses constant 83 ms timing. PNG frames retain full RGB; one shared palette keeps GIF backgrounds consistent.

Frame-zero endpoints match across states. Hooks may switch at any point in an idle/working loop, and device uploads introduce delay, so this does not establish frame-perfect live transitions. Native contacts and head crops were inspected; owner review of these three new animations is pending.

Point a bridge config's `pack` at this `review` directory to use it. It includes idle and these three new clips; absent states stay optional under the existing bridge rules. This revision does not change the currently running preview config or upload the new artwork.
