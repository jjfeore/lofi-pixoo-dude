# Delegation: descending return and neon-green indicator

The [combined preview](review/delegation-cycle-preview.gif) plays dispatch, delegated work and return. Individual previews: [dispatch](review/delegating-start-preview.gif), [working loop](review/delegating-preview.gif), [return](review/delegating-finished-preview.gif).

This refines the approved [v2 artwork](../delegation-v2/README.md) in two places. After downloading and turning off its lamp, the returning drone descends below the window instead of departing right. The ten-frame descent exactly reverses the dispatch ascent's vertical positions. The task indicator in both transfers is saturated neon green (`#30FF00`), with a brighter green upload acknowledgment (`#90FF18`). The inactive lamp remains unchanged.

Dispatch and return retain forty frames each; the delegation loop retains thirty-two. All use 83 ms per frame, approximately 12 FPS. Arrival, hover, transfer timing, rear-display patterns, working character, typing and main-screen animation remain unchanged. Dispatch still exits right. Transfer clips still omit car traffic. The complete working loop and the eight earlier physical clips keep their exact PNG/GIF exports. The 112-frame combined GIF is for desktop review; individual device clips remain within forty frames.

## Sources and reproduction

[authoring.json](authoring.json) references v2's generated drone and approved palette. No new raster art or character generation is needed. The offline compositor supports an optional `drone.finish_exit` of `bottom`, plus configurable received/acknowledgment colors; omitted fields retain v2 behavior. To preserve the new one-pixel green light in GIF previews, unused per-frame palette slots carry the two exact indicator colors. Every other preview pixel keeps its original palette color.

From the repository root, choose a fresh output directory:

```powershell
cargo run --locked --release --example delegation_revision -- pets/decker/revisions/delegation-v3/authoring.json local/delegation-v3-rebuild
python pets/decker/revisions/delegation-v3/verify.py local/delegation-v3-rebuild
```

The independent [verifier](verify.py) checks native source preservation, flight/transfer/lamp phases, descent symmetry, exact active-light colors in PNGs and GIFs, unchanged loop assets, and that transfer changes are limited to the indicator or moving drone. It also checks prior clips, whole-screen animation, typing, GIF timing, sprite order and native/enlarged matching. Results are saved in [verification.json](review/verification.json).

The offline helper compiled, and all image checks passed. A separate v2 reproduction kept all 461 PNG/GIF/manifest/motion files byte-identical with the new optional settings omitted. Native window and full-scene contacts were inspected; owner review of v3 remains pending. Direct bridge CLI validation was attempted but blocked by the existing sandbox directory-access error (`Access is denied`, OS error 5). Formatting and Clippy retain the previously documented sandbox limitation and were not rerun for this artwork refinement.

This is an artwork-only revision. Bridge lifecycle behavior and the prior runtime verification remain as documented in v2. The existing [Pixoo upload-delay investigation](../../../../docs/playback-investigation.md) still applies. No bridge restart, settings change, device upload or commit is included.

For external formatting, Clippy and bridge pack validation:

```powershell
.\scripts\check-artwork.ps1 -Example delegation_revision -Pack pets/decker/revisions/delegation-v3/review -Format
```
