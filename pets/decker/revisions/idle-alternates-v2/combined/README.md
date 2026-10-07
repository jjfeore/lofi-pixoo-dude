# Complete bridge pack

This is the runtime pack for the four approved alternate idle animations. It combines the complete `delegation-v3/review` pack with `idle-flyby`, `idle-yawn`, `idle-message`, and `idle-city` from `idle-alternates-v2/review`.

All twelve previous manifest entries and their sprite sheets are preserved exactly, including work entry, finish, interruption, attention, both compaction poses, and delegation entry/loop/exit. The four alternate sprite sheets also match the approved review files byte for byte. Every animation uses the accepted 83 ms frame clock. Each alternate has forty frames and plays for 3.320 seconds.

Point the bridge configuration's `pack` at this directory and list the four alternate names under `[idle_alternates]`. The examples use a random 45–60 second interval and avoid immediately repeating the same alternate. The bridge returns to normal idle after each alternate, and activity interrupts it.

`verification.json` records the copied source hashes and the 9,306,112-byte prepared frame payload, within the bridge's 16 MiB pack limit. The pack has sixteen manifest entries and fifteen unique physical sprite sheets; `compacting` is a variant alias. Previews and artwork authoring files remain in the sibling `review` directory.
