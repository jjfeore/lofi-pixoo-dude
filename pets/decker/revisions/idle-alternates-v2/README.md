# Revised alternate idle candidates

All four candidates now use the accepted **83 ms frame clock (approximately 12 FPS)**. Each has forty native 64×64 RGB frames and lasts **3.320 seconds**, within the forty-frame limit. The review manifest marks the alternates as single-play clips; GIF previews repeat for inspection.

The artwork is approved. The [complete runtime pack](combined/README.md) adds these four clips to all existing delegation-v3 lifecycle animations without changing their sources. The bridge now supports configurable random alternate-idle waits; see [configuration and restart instructions](../../../../docs/configuration.md#alternate-idle-animations). Use `combined/` for live operation; `review/` contains the idle-only artwork review set.

| Candidate | Revisions |
| --- | --- |
| [Window flyby](review/idle-flyby-preview.gif) | The close hovercar is clipped along Decker's native head/raised-visor outline instead of disappearing at the old straight boundary. The monitor sweep is retained. Four background vehicles have staggered departures, opposing directions, different lanes and speeds. |
| [Sleepy Decker](review/idle-yawn-preview.gif) | The reviewed yawn remains. Amber recoloring is restricted to the actual two-column lamp bar, leaving its surrounding glow unchanged. Background traffic follows a separate three-vehicle pattern. |
| [Incoming message](review/idle-message-preview.gif) | The yellow notification dot is removed. The envelope grows from 9×5 to 10×6 pixels, with a larger outline and flap, and is shifted one pixel right and one pixel down on the secondary monitor. Main-screen message scrolling is retained, with a third traffic pattern. |
| [Night city](review/idle-city-preview.gif) | Rainbow recoloring is restricted to the lamp bar. Floor lights follow the existing window rows in the middle building. The elevator is one pixel on the exposed right-tower shaft and disappears behind the nearer building. Traffic follows a fourth pattern. |

The [four-way GIF](review/alternates-overview.gif) places flyby and yawn above message and city. [Key poses](review/key-poses.png) show frames 0, 10, 20 and 30, at 0.00, 0.83, 1.66 and 2.49 seconds. Native GIFs, enlarged GIFs, contact sheets, numbered PNGs and sprite sheets are in `review/`. The inline comparison adds synchronized playback, pause, scrubbing and actual-size viewing at the exact 83 ms clock.

## Scene geometry and preservation

Coordinates refer to the native 64×64 grid, starting at zero. The lamp is columns 4–5, rows 15–24. The close car crosses rows 17–21: visible glass begins at column 31 on rows 17–20 and column 32 on row 21. The window's right boundary remains column 59. This lets the car travel farther left and disappear progressively behind the actual foreground outline while preserving the head and visor pixels.

The middle building's existing window rows are at rows 23, 26 and 29, columns 41–42. The earlier floating floor rectangles are removed. The elevator follows column 49 from row 16 downward. Its visible shaft ends at row 26; the path continues behind the nearer building from row 27, where the light is occluded. No elevator pixels are drawn over that foreground building.

Each traffic profile reuses the accepted cyan, magenta and distant-car sprites, with independent one-pass paths and no wrapping. Existing cars are removed using the approved clean scene. Lower lanes are clipped behind the head silhouette. All vehicles start and finish out of view, and every alternate begins and ends at the canonical idle frame.

All other pixels match the corresponding accepted idle source frame, preserving body, arms, hands, visor and the existing ambient monitor motion except for the stated screen effects. Normal idle exports are copied byte-for-byte from `delegation-v3/review`. The hovercar and eyelid art reuse the generated [v1 sources and prompts](../idle-alternates-v1/sources/prompts.json). The mouth/chin poses continue to reuse the accepted `interrupted-v4` rasters. V1 remains available for comparison.

## Reproduction and checks

The [authoring plan](authoring.json) supplies geometry, traffic paths and timing to the [offline compositor](../../../../examples/idle_alternates.rs). Choose a fresh output directory:

```powershell
cargo run --locked --offline --release --example idle_alternates -- pets/decker/revisions/idle-alternates-v2/authoring.json local/idle-alternates-v2-rebuild
python pets/decker/revisions/idle-alternates-v2/verify.py local/idle-alternates-v2-rebuild
```

The [read-only verifier](verify.py) checks the source frame count, dimensions, sprite ordering, all manifest frame clocks, actual traffic pixels and monotone paths, bounded lamp pixels, flyby silhouette clipping, enlarged envelope geometry, single-pixel elevator descent and foreground occlusion, exact preservation outside the declared layers, canonical endpoints and byte-identical normal idle exports. Each intended layer must visibly change.

GIF supports only 10 ms ticks. New GIFs therefore use cumulatively quantized 80/90 ms frame delays, totaling exactly 3,320 ms. Native PNG/sprite playback and the inline preview use exactly 83 ms per frame. Each GIF uses one fixed palette for its complete sequence; native RGB sources retain full color. Native and enlarged GIF pixels are checked for agreement.

The envelope centering change preserves the existing message and four-way GIF palettes, saved as [color metadata](sources/preview-palettes.json), so the shift does not recolor the surrounding scene in those previews. Every native pixel outside the secondary monitor and all other clips remain identical to the preceding v2 export. The [centering comparison](review/message-centering-verification.json) verifies those exact pixels in all forty frames and both affected GIFs; the [focused preview checks](review/message-centering-preview-verification.json) verify playback, scrubbing, actual-size viewing and timing.

The release compositor and artwork checks passed; results are in [verification.json](review/verification.json). The inline preview's JavaScript and controls have separate offline checks in [preview-controls-verification.json](review/preview-controls-verification.json), including the 83 ms frame boundaries and 3,320 ms repeat. Browser rendering is not included in those checks. Bridge CLI validation reported `pack directory not found: Access is denied. (os error 5)` under the Windows sandbox. Physical playback has not been tested for this review revision. Rebuilding the original v1 authoring plan also passed its existing verifier.

These files are artwork for approval. Random selection after 45–60 seconds of uninterrupted idle remains the next bridge step after artwork approval. The active pack, bridge process, configuration and device were not changed.
