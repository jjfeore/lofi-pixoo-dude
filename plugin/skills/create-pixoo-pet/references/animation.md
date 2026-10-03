# Motion for a fixed 64x64 display

The display has no pointer, drag position, or unread-message UI. Build actions that read from pose, monitor color, and a few distinct pixels. A complete room can work if most of it stays still.

| Clip | Lifecycle meaning | Example motion |
| --- | --- | --- |
| idle | No observed active work | Slow typing, breathing, subtle monitor scrolling |
| working-enter | Optional transition on aggregate work starting | Lower visor, illuminate indicator |
| working | At least one observed active turn | Faster typing, bright cyan screen, visor down |
| needs-input | Observed permission request | Amber indicator and a large monitor symbol |
| finished | Completion notifier after the last active chat ends | Raise visor and soften screen light |
| interrupted | Interrupt after the last active chat ends | Brief restrained red reaction, then raise visor |
| compacting | Between PreCompact and PostCompact | A few blocks converge or a progress symbol on the second monitor |
| delegating | Observed child activity | Green strokes type on the second monitor |

Use these as examples, adapting to the chosen pet. Do not reproduce a terminal's tiny text. A second monitor is useful for independently readable activity, but runtime sends whole frames, so exported clips must include their background.

For a scene with a visor or other base pose, compaction can occur while idle too. Author `compacting-idle` and `compacting-working` and reference them through the compaction clip's `variants`. Delegation and needs-input generally accompany work but still need a self-contained readable pose. Simultaneous activities use priority: needs-input, compaction, delegation, work, idle. The bridge selects one full clip rather than composing layered activities.

Start with 6-12 frames for transitions and 12-24 for loops, using fewer if enough. Each cell is 64x64. Cell count and ordering must agree with the manifest. Frames should not flicker from background drift or rescaled camera framing. Compare each to the approved base and inspect the actual downsampled output, not just the generation source.

Finished and interrupted clips use host timing to return to the current base animation. Hardware upload and visible-start delays can make that timing approximate. Avoid making a narrative depend on a frame-perfect transition. Fast red flashes are unnecessary; a small red indicator and a brief pose reaction suffice.
