# Codex integration

Start the bridge before new Codex work. Global hooks cover all observed local chats on that execution host. The skill/plugin creates artwork; the separately running bridge handles lifecycle events and the display.

## Prepare a merge

Create a private `local/bridge.toml` from `config/bridge.example.toml`. Set the device address, optional token, pack path and pipe. Pack paths are relative to that config. From the bundle or repository:

```powershell
.\scripts\prepare-codex-setup.ps1 -Config local\bridge.toml -OutputDirectory local\codex-setup
```

The script calls the executable's `setup` command. It stages three proposed files and exact snapshots of existing destination files. It reads `$CODEX_HOME` or the normal user `~/.codex` directory; use `-CodexHome 'C:\another-codex-home'` to choose one explicitly. `-Executable` selects another built executable. The staging directory must be new and should be ignored by source control: it can contain private user settings and the bridge's token.

Preparation preserves unrelated hooks and settings, keeps TOML comments where practical, enables `[features] hooks = true`, installs applicable Pixoo handler groups, and sets the documented completion adapter. An existing Codex `notify` array is copied into the bridge's `notification_forward`, preserving its whole original argument list. Repeated preparation recognizes the Pixoo adapter and keeps the original forwarding command. If an independently changed notifier conflicts with an already configured forwarding array, preparation stops for explicit configuration instead of discarding one.

Existing Pixoo groups are replaced or removed when applicable states/paths change, without accumulating duplicates. Other handlers remain. Trust entries are preserved as data; the setup never manufactures trust for new or changed commands.

## Review and apply

Inspect `config.toml`, `hooks.json`, and `bridge.toml` in the new review directory. `original-*` files contain the prior versions. Check-only and apply commands are:

```powershell
.\scripts\apply-codex-setup.ps1 -PlanDirectory local\codex-setup
.\scripts\apply-codex-setup.ps1 -PlanDirectory local\codex-setup -Apply
```

The installer checks SHA256 hashes of both destinations and proposals. It refuses intervening edits, missing originals, or newly appeared files. On apply, it backs up existing files into a timestamped directory, copies proposals, and attempts rollback if copying fails. Run it in your own PowerShell when an agent's sandbox cannot write global Codex files.

Review/trust the new or changed commands using Codex's normal flow and restart/reload Codex as needed. Restart the bridge after its config changes. Test a new prompt/completion, approval/resumption, interrupt, compaction, and actual subagent work. Installed JSON and trust alone do not prove live event delivery. `pixoo-pet status --pipe '\\.\pipe\pixoo-pet'` exposes observed counts and selected clips without conversation content.

Use a fresh review directory for each update. To restore, stop the bridge and copy backed-up `config.toml` and `hooks.json` to the Codex home and `bridge.toml` to its original private path. If a destination did not exist before installation, it has no backup; remove only that newly created file. Review subsequent settings edits before restoring an older backup. There is no scheduled task or service to remove.

## Manual integration

```powershell
.\pixoo-pet.exe hooks --config .\local\bridge.toml --output .\local\pixoo-hooks.json
```

This writes only a fragment and prints the completion `notify` command array. Merge its groups into the global `hooks.json`, preserving other groups, ordering and other fields. If no file exists, the fragment can become that file. Enable `[features] hooks = true` as required by the [official hook guide](https://learn.chatgpt.com/docs/hooks).

For completion, first copy the whole previous `notify` array into the bridge's top-level `notification_forward`, above `[device]`. Set Codex's notify to the printed array, for example:

```toml
notify = ['C:\pixoo\pixoo-pet.exe', 'notify', '--config', 'C:\pixoo\local\bridge.toml']
```

Codex appends the original notification JSON as one argument; the adapter forwards that argument to the previous notifier and sends only normalized lifecycle metadata to the bridge. Never put the Pixoo adapter itself into `notification_forward`, which would recurse. Completion uses the [official notifier](https://learn.chatgpt.com/docs/config-file/config-advanced#notifications); Stop handlers can request continuation and are not treated as final completion.

The adapter sends to the bridge before launching the preserved notifier. Completion delivery retries transient pipe failures at most three times, with a 150 ms budget per attempt and 20 ms between attempts. Permission failures are not retried. Normal `notify` calls still return harmless success on a delivery failure, but report it on stderr; add `--strict` when diagnosing delivery to get a failing exit code. Notification contents are never included in these diagnostics.

Bridge status includes `completion.received`, `completion.applied`, and `completion.last_applied`. These count notifications received since bridge startup and those matching the session's current turn. A duplicate or stale notification can be received without being applied. If work stays visible after Codex finishes, a missing receipt points to the notifier/pipe path; a receipt with `last_applied = false` points to duplicate or mismatched turn metadata. An applied completion may leave Decker working when another observed chat is active. The bridge log records the completion disposition and aggregate work/child counts without conversation content or identifiers.

## Hook scope and cost

SessionStart, SessionEnd, UserPromptSubmit and Interrupt always provide core bookkeeping. PreCompact/PostCompact and SubagentStart/SubagentStop are generated only when their mapped clips exist. Needs-input artwork adds PermissionRequest and narrowly matched PostToolUse resumption handlers. Generate setup again after adding/removing these optional clips.

Emitters read official stdin JSON, keep only session/turn/tool/agent identifiers, send metadata over the current-user/logon named pipe and return valid no-op `{}`. They make no device requests. Simple Windows paths use a one-second outer timeout; paths requiring the PowerShell wrapper use three seconds. The internal pipe round trip has a 50 ms budget. Windows process/shell launch adds overhead; the timeout does not bound OS scheduling or launch time. HTTP failure affects the persistent bridge. There are no web-search or general tool-activity animations.

Codex inherits the session shell for command hooks. Generated `commandWindows` handlers use unquoted arguments when both the path and pipe contain only safe characters; these commands work in PowerShell and `cmd.exe` with no additional process. Paths requiring quoting use an encoded PowerShell command that explicitly invokes the emitter and preserves its stdin, including spaces and apostrophes. A bare `"C:\path\pixoo-pet.exe" emit ...` command fails in PowerShell because the quoted path is a string expression. The encoded fallback contains only the executable path and pipe; it contains no hook payload or device credentials. That fallback adds one PowerShell process per hook, so check launch time on its host, especially for Interrupt's three-second maximum.

After upgrading from the old command format, prepare/apply a fresh setup and review/trust the changed Windows handlers. Existing trust hashes are preserved, so changed handlers require review through Codex's normal flow. The running bridge's GIF cache and display playback do not need to change for this hook-launch fix.
