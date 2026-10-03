# Cyberpunk decker

This is the first implementation's artwork baseline. The owner confirmed correct playback and animation on the physical display, and requested image refinements after the initial code commit. Preserve these source grids and prompts for that next pass.

The canonical base is `base-v2.png`, approved by the owner on 2026-10-02 after moving the second monitor to the rear desk and tightening the foreground crop. Earlier base drafts and generation prompts are retained under `sources/` for revisions.

The display pack supplies idle, working, needs-input, finished, interrupted, compaction and delegation, plus a working-entry transition. The visor stays down during sustained work. Finished reverses the jack-in sequence and ends at the exact approved idle base. Interrupted adds the generated red warning/recoil, then returns to that same idle endpoint. Compaction has separate idle and working variants so the visor pose remains coherent.

Generation produced 16-cell source grids. Eight representative frames per uploaded animation were selected to limit transfer time, at 83 ms per frame (about 12 FPS, 0.664 seconds per cycle). The complete room is always exported as RGB. Rectangular extraction masks freeze the approved background while allowing the requested character/monitor regions to change. Contact sheets and enlarged GIFs are in `previews/`; GIF previews quantize timing to their 10 ms clock, while the bridge uses 83 ms.

Source grids, numbered prepared frames and prompts allow refinement without inventing new identities. The manifest is the authoritative selection; not every authoring frame must be uploaded. `previews/*-sprite.png` are native-cell PNG sheets for reuse, and `pet.json` points at those prepared sheets when present. A complete pose is preserved outside the explicitly animated regions. This is a first animation pack, with direct hardware visual review still useful for screen contrast and switching/loading behavior.
