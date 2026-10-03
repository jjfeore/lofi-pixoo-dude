# Pixoo pets for Codex

This repo provides a reusable **create-pixoo-pet skill/plugin** and a small **Rust bridge for Windows**. The bridge follows documented local Codex lifecycle hooks and sends 64x64 RGB animations to a Divoom Pixoo64 over its LAN HTTP API.

The executable needs no Python or Node installation. Artwork is decoded and encoded once at startup. Stable clips loop on the display; the bridge sleeps between events instead of rendering or sending 12 frames per second.

## Run

From a release bundle, use `pixoo-pet.exe`. From source, install the stable Rust MSVC toolchain and Visual Studio C++ build tools, then:

```powershell
cargo build --locked --release
.\target\release\pixoo-pet.exe validate .\pets\decker
.\target\release\pixoo-pet.exe run --config .\config\bridge.example.toml --dry-run
```

Copy the example config into a private local directory and set `device.address` to your Pixoo IP or hostname. `device.local_token` is optional and is sent as `LocalToken` when configured. Brightness is preserved unless explicitly configured. Pack paths are relative to the config file. See [configuration](docs/configuration.md).

```powershell
.\pixoo-pet.exe probe --config .\bridge.toml
.\pixoo-pet.exe run --config .\bridge.toml
.\pixoo-pet.exe status --pipe '\\.\pipe\pixoo-pet'
```

Keep the bridge running. Install the generated hooks and completion adapter following [Codex setup](docs/codex-setup.md). Existing hooks and any completion notifier must be preserved. Closing the bridge leaves the panel's current content in place; hook emitters remain harmless when the bridge is unavailable. The Divoom app can select another channel after shutdown.

## Install or update Codex integration

Create your private config, set its device address/token, and choose a pet:

```powershell
New-Item -ItemType Directory -Path .\local -Force
Copy-Item .\config\bridge.example.toml .\local\bridge.toml
# Edit local/bridge.toml before using your display.
.\scripts\prepare-codex-setup.ps1 -Config local\bridge.toml -OutputDirectory local\codex-setup
```

Review the proposed `config.toml`, `hooks.json`, and `bridge.toml` in that new directory. Preparation changes no destination files. It preserves other hooks/settings and copies any existing completion notifier into `notification_forward` automatically. Then install:

```powershell
.\scripts\apply-codex-setup.ps1 -PlanDirectory local\codex-setup -Apply
```

The installer checks for intervening edits, saves originals in the review directory, and attempts rollback if installation fails. Review/trust new or changed hooks in Codex, and restart/reload Codex if needed. Repeat preparation with a **new review directory** after changing paths, optional clips or your hook setup; existing Pixoo handlers are updated without duplicates. See [Codex setup](docs/codex-setup.md) for manual installation, restoring backups and custom Codex homes. Run these scripts in your own PowerShell when an agent sandbox cannot edit global settings.

Start in a terminal with `pixoo-pet.exe run --config local\bridge.toml`, or use `scripts/start-bridge.ps1` for a hidden process. The scripts find the release executable at the bundle root or `target/release/` in a source checkout. They do not install a service or automatic startup task.

## Create or add a pet

The portable plugin is in [plugin](plugin/plugin.json). Its entrypoint is [create-pixoo-pet](plugin/skills/create-pixoo-pet/SKILL.md). It supports characters on black and full room scenes. It shows the real 64x64 base and waits for approval before creating the complete animation set.

Use the skill through your Codex skill/plugin installation route, or ask Codex to read its workspace path explicitly. Skill-only installation can copy `plugin/skills/create-pixoo-pet` into your Codex skills directory. The package does not install or start the bridge automatically. Plugin installation across Codex surfaces is separate from the executable setup.

The included cyberpunk decker pack uses the approved base and supplies the full selected animation set, with idle/working compaction variants. Existing images work without image generation:

```powershell
.\pixoo-pet.exe import .\pets\my-pet --id my-pet --frame-ms 83
.\pixoo-pet.exe validate .\pets\my-pet
.\pixoo-pet.exe preview .\pets\my-pet working --output .\previews\working.gif --contact .\previews\working.png
```

Supported sources are static PNGs, 64-pixel sprite grids, numbered PNG frame folders, and 64x64 GIFs. Point `pack` at the imported directory. All clips are optional, and state names can be remapped. The [pack format](plugin/skills/create-pixoo-pet/references/pack-format.md) is shared by the bridge and skill. The bundled `diagnostic` pack is deliberately simple transport-test artwork. The `decker` pack includes the approved personal scene and full selected set.

## What the display follows

| Selection | Source |
| --- | --- |
| idle | No observed active turn |
| working | UserPromptSubmit; observed activity after starting the bridge |
| needs-input | PermissionRequest, cleared by matching observed tool completion or terminal event |
| finished | Official `agent-turn-complete` notification |
| interrupted | Interrupt |
| compacting | PreCompact / PostCompact |
| delegating | SubagentStart / SubagentStop |

All observed local chats are aggregated. Visible priority is needs-input, compaction, delegation, working, idle. An absent clip skips that visual selection; lifecycle bookkeeping still runs. A chat finishing does not jack out the character while another chat is still working. Optional working entry runs once when aggregate work starts. Finish/interruption reactions return to the current base after their estimated duration.

These are observations, not complete native-pet parity. Ordinary conversational questions have no universal waiting hook, SubagentStop can be followed by continuation, and restarting the bridge does not reconstruct already-active chats. `Stop` is deliberately not treated as final completion. See [coverage and limitations](docs/limitations.md).

On the development Windows host, the complete decker pack used about 8.6 MiB of resident memory and no measurable CPU during a 20-second quiet sample. Final hook shell invocations had a 52 ms median and 67 ms p95; direct launches had a 23 ms median. Eight-frame personal clips took a median 3.19 seconds to upload, with a 5.95-second slowest upload. Single-play gestures can repeat while the next clip loads. These are measurements on one host/device, not universal performance guarantees; see [verification](docs/verification.md).

## Development and distribution

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python tests\integration.py -v
python scripts\benchmark.py
powershell -File scripts\package.ps1
```

Python is used only by development test/measurement scripts. The integration harness exercises the actual executable, Windows pipes, concurrent emitters, and a mock HTTP device. Linux/macOS can use asset and replay commands; this release's live bridge transport is Windows-only. A Windows release archive is produced under `dist/`. No project license has been selected yet; choose release terms before public distribution. Third-party dependencies retain their own licenses.

Research and project decisions are under `.planning/`, which is locally ignored by the repository's existing convention. Authoritative interfaces: [Codex hooks](https://learn.chatgpt.com/docs/hooks), [completion notifications](https://learn.chatgpt.com/docs/config-file/config-advanced#notifications), [portable plugin packaging](https://developers.openai.com/plugins/build/plugins), and [Divoom's Pixoo API guide](https://divoom.com/blogs/app-guide/pixoo-64-api-beginner-guide).
