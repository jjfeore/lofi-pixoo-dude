# Delegation: drone dispatch, falling glyphs and return

The [combined preview](review/delegation-cycle-preview.gif) plays the three clips in order. Individual previews: [dispatch](review/delegating-start-preview.gif), [delegated work](review/delegating-preview.gif), [return](review/delegating-finished-preview.gif).

| Clip | Frames | Native duration | Motion |
| --- | --- | --- | --- |
| delegating-start | 40 | 3.320 s | Drone rises outside the window, hovers above the rear monitor's upload arrow, acknowledges with a bright flash/green lamp, then departs right |
| delegating | 32 | 2.656 s | Green falling glyphs on a black rear screen, with three distinct traffic profiles |
| delegating-finished | 40 | 3.320 s | Drone returns from the right, hovers for a pulsing download arrow, clears its lamp and departs; the rear waveform returns |

All use 83 ms per frame, approximately 12 FPS. The decker keeps typing with the visor down and the entire visible main screen animated. Transfers omit cars. The 112-frame combined GIF is a desktop review sequence; device clips individually respect the forty-frame ceiling. GIF centisecond timing makes the loop 2.650 s and the combined preview 9.290 s; native combined timing is 9.296 s.

## Artwork and sources

[authoring.json](authoring.json) configures masks, source projection, lamp, flight/transfer phases and traffic. Built-in image generation supplied the transparent [drone source](sources/drone-generated.png), with its saved [prompt](sources/drone-prompt.txt). The offline [Rust compositor](../../../../examples/delegation_revision.rs) projects it once to 17x8 pixels with a binary alpha mask. Its fixed pose is translated behind the window/foreground monitors; only its registered one-pixel lamp changes. Black/green falling glyphs and geometric arrows are native monitor content. Accepted working frames supply all character, visor, typing and main-glass motion.

The [review manifest](review/pet.json) also retains the eight prior physical clips exactly: 264 native frames, sixteen individual GIFs, all prior sprites/previews, manifest selections and motion metadata. Nothing in those accepted animations is regenerated. Native drone, window, rear-display and full-scene contacts were inspected; owner review of delegation remains pending.

## Bridge behavior

The delegation loop references optional `entry: delegating-start` and `exit: delegating-finished`. The first observed child across local chats dispatches the drone; the last observed child stop triggers its return. Multiple or duplicate child events do not replay those transfers. Missing transition references retain the previous loop-only behavior; a missing delegation loop skips its visuals while bookkeeping continues. Needs-input and compaction can preempt transfers. Parent completion/interruption clears children without synthesizing a drone return, and the delegation finish returns to the current base while the parent continues working.

Use the updated source-built executable for `exit`; older binaries reject that field. SubagentStop describes an observed stop and can be followed by continuation. Short jobs, priority changes and device upload delays can replace or skip transfers. Playback remains timed on the host after upload acknowledgment, without a frame-perfect visible-start guarantee. The new transitions are optional if switching delays on your firmware make them distracting. No new hooks or polling are required. Frames are prepared once; these changes add event-driven selections rather than continuous streaming or runtime image generation.

The existing [playback investigation](../../../../docs/playback-investigation.md) found current raw uploads too slow for short transitions; this artwork/runtime revision does not replace that transport. Keep the transfer art for future native-file switching, or remove `entry` and `exit` from a separate playback manifest to use only the loop. Empty configuration mappings for the transition names do not disable direct manifest references. An empty `animations.delegating` mapping disables delegation visuals altogether.

## Reproduce and verify

From the repository root, choose a fresh directory:

```powershell
cargo run --locked --release --example delegation_revision -- pets/decker/revisions/delegation-v2/authoring.json local/delegation-v2-rebuild
python pets/decker/revisions/delegation-v2/verify.py local/delegation-v2-rebuild
```

The independent verifier needs Python/Pillow only for development. The bridge has no Python runtime dependency. Artwork/example and bridge release builds passed. [Image evidence](review/verification.json) passed all 112 frames, exact source/drone projection, clipping/flight/lamp phases, stable hover, pulsing arrow shapes, falling glyphs, traffic policy, fixed hands/bezel, 209 animated main-glass pixels, prior artifact preservation, sprite/GIF ordering, shared preview colors and rear-pattern joins. Prepared frame data is approximately 6.38 MiB including manifest aliases.

[Runtime evidence](review/runtime-verification.json) records 21 passing Rust tests, including first/last child aggregation, duplicates, rapid stop/restart, missing artwork, priority preemption, parent interruption and exit-reference compatibility/validation. Four existing filesystem tests failed on sandbox directory-access errors and were excluded from the final successful run. Formatting, Clippy, CLI pack validation and the new local mock integration test were attempted but blocked by the same access restriction. Python and PowerShell syntax checks passed. Those blocked checks are not marked passed.

To complete them in your own PowerShell:

```powershell
.\scripts\check-artwork.ps1 -Example delegation_revision -Pack pets/decker/revisions/delegation-v2/review -Format
python tests/integration.py Integration.test_delegation_entry_loop_and_exit_use_first_and_last_child
```

For this example the helper also checks and builds the updated runtime before validating the pack. This revision does not activate the review pack, edit live settings/hooks, upload to the Pixoo or create a commit.
