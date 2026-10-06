# Interrupted: dumpshock and a getaway chase

Superseded by [interrupted-v3](../interrupted-v3/README.md). Owner review requested a stable face outline with an opening mouth and three bolder visor rays. This revision and its generated expression study remain reproducible as historical source material.

The [interruption preview](review/interrupted-preview.gif) contains forty native 64x64 frames at 83 ms, approximately 12 FPS and 3.320 seconds. A brief red screen glitch and jaw wince/gasp suggest dumpshock. One magenta getaway car moves across the window, followed by five blue-grey patrol cars with alternating red/blue roof lights. The visor lifts automatically along the accepted finish path. Red screen/visor lighting settles back to idle, and the rear warning returns to the green oscilloscope.

The [context preview](review/interruption-cycle-preview.gif) plays working, interruption and idle together. Its 104 frames are a desktop review sequence; the device interruption clip has forty frames and `loop: false`. The first interruption frame is exactly the approved working start, and its last frame exactly matches the idle start.

The [review pack](review/pet.json) includes the new interruption and all seven previously approved physical clips. Those 224 native frames, fourteen individual GIFs, sprites, manifest selections and previous motion metadata are preserved exactly. Owner visual review of the new clip is pending.

## Artwork and motion

[authoring.json](authoring.json) configures the forty-frame timeline, full main-glass outline, expression mask, red/rear recovery timing, vehicles, lanes and beacon phases. The offline [Rust compositor](../../../../examples/interruption_revision.rs) prepends eight shock frames to the accepted thirty-two-frame motorized finish. Both arms and hands retain their corresponding approved pose and typing pixels throughout. There is no hand-operated visor gesture.

The original interrupted contact sheet supplied the reaction mood. Built-in image generation produced a [four-expression study](sources/expression-study.png) from the accepted working image; its exact [prompt](sources/expression-prompt.txt) is saved. Only an 8x8 lower-jaw patch from the wince/gasp cells appears in five prelude frames, using colors from the canonical face region. The hair, eyes, visor, hands and room retain accepted source pixels. Normal endpoints use the actual approved frames, not generated neutral poses. Native [expression contacts](review/expression-study-contact.png) retain the complete study for inspection.

All 209 main-glass pixels animate. The early corruption scrolls contents inside each traced screen column and adds brief scan-line accents; it never moves the bezel. Red lighting is confined to the actual monitor glass and moving visor. Small electrical marks near the temple appear in three shock frames. The rear screen has a red octagonal halt symbol with a white bar, readable in eleven by seven pixels. It fades to the accepted moving waveform as the character recovers.

The chase is one continuous pass per vehicle, with no looping position reset or overtaking. Patrols follow the leader in staggered lanes; all six can be visible together. Existing small transparent car artwork supplies the silhouettes. Code-native police roof lights alternate every two frames with per-car phase offsets. The sky mask preserves city/window/foreground pixels outside the established traffic region.

## Reproduce and verify

From the repository root, choose a new output directory:

```powershell
cargo run --locked --release --example interruption_revision -- pets/decker/revisions/interrupted-v2/authoring.json local/interrupted-v2-rebuild
python pets/decker/revisions/interrupted-v2/verify.py local/interrupted-v2-rebuild
```

Compilation, export and [independent saved-image checks](review/verification.json) passed forty distinct frames, canonical endpoints, exact approved motorized geometry, fixed body/arms/hands outside the expression/prop regions, full-glass motion, 214 fixed bezel-region pixels, initial red visor/screen, recovered colors/waveform, forward pursuit and visible beacon cycles, exact prior pack artifacts, sprite ordering and native/enlarged GIF matching. [Motion records](review/interruption-motion.json) save source frames, pose progress, red levels and per-vehicle positions/lights. [Scene](review/interrupted-contact.png), [head](review/interrupted-heads.png) and [rear-screen](review/interrupted-rear-contact.png) contacts were inspected. Region comparisons verify preservation against accepted images; visual expression quality still needs owner review.

One palette is fitted across the complete context sequence, keeping fixed preview colors stable. Device PNG sprites retain full RGB. Individual GIF timing totals 3.320 seconds with 80/90 ms delays. The context GIF lasts 8.630 seconds; native context timing would be 8.632 seconds.

## Remaining tool checks

Rust formatting, Clippy with warnings denied and bridge CLI validation were attempted but rejected by sandbox directory-access errors. They are not marked passed. The updated, syntax-checked helper accepts this new offline example. Run it in your PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example interruption_revision -Pack pets/decker/revisions/interrupted-v2/review -Format
```

This checks the artwork example and validates the pack; it does not upload or edit device/bridge settings. No bridge runtime build, live activation or commit accompanies this revision. Delegation remains for the next artwork revision.
