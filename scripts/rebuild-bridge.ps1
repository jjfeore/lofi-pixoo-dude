# Build and validate first, then replace only the bridge for this executable/config.
# Run this in the owner's PowerShell, outside the Codex filesystem/pipe sandbox.
[CmdletBinding()]
param(
    [string]$Config = 'local\bridge.toml',
    [string]$Executable,
    [string]$PendingConfig,
    [switch]$CheckOnly,
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'common.ps1')
if (-not (Test-Path -LiteralPath (Join-Path $taskRepoRoot 'Cargo.toml') -PathType Leaf)) {
    throw 'Rebuilding requires the source checkout with Cargo.toml and the Rust toolchain.'
}

function Resolve-BridgePath([string]$Path) {
    if (-not [IO.Path]::IsPathRooted($Path)) { $Path = Join-Path $taskRepoRoot $Path }
    return [IO.Path]::GetFullPath($Path)
}

function Test-BridgeCommand([string]$CommandLine, [string]$ConfigPath) {
    # Require the run subcommand and an exact absolute config argument. Other
    # configs and short-lived emit/notify processes must never be stopped.
    $runPattern = '^\s*(?:"[^"]+"|\S+)\s+run(?:\s|$)'
    $configArgument = '"' + [regex]::Escape($ConfigPath) + '"(?=\s|$)'
    if ($ConfigPath -notmatch '\s') {
        $configArgument = '(?:' + $configArgument + '|' + [regex]::Escape($ConfigPath) + '(?=\s|$))'
    }
    $configPattern = '(?:^|\s)(?:--config|-c)(?:=|\s+)' + $configArgument
    return $CommandLine -and $CommandLine -match $runPattern -and $CommandLine -match $configPattern
}

function Get-BridgeStatus([string]$Tool, [string]$Pipe) {
    $savedPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & $Tool status --pipe $Pipe 2>$null
        if ($LASTEXITCODE -eq 0) { return ($output -join "`n" | ConvertFrom-Json) }
    }
    catch { return $null }
    finally { $ErrorActionPreference = $savedPreference }
    return $null
}

function Start-BridgeProcess([string]$Tool, [string]$ConfigPath, [string]$LogPath, [bool]$DryRun = $false) {
    $arguments = @('run', '--config', ('"{0}"' -f $ConfigPath))
    if ($DryRun) { $arguments += '--dry-run' }
    return Start-Process -FilePath $Tool -ArgumentList $arguments `
        -WorkingDirectory $taskRepoRoot -WindowStyle Hidden -PassThru `
        -RedirectStandardError $LogPath -RedirectStandardOutput ($LogPath + '.stdout')
}

$taskConfigPath = Resolve-BridgePath $Config
if (-not (Test-Path -LiteralPath $taskConfigPath -PathType Leaf)) { throw "Config not found: $taskConfigPath" }
if (-not $Executable) {
    $Executable = if (Test-Path -LiteralPath (Join-Path $taskRepoRoot 'pixoo-pet.exe')) {
        'pixoo-pet.exe'
    } else { 'target\release\pixoo-pet.exe' }
}
$taskExePath = Resolve-BridgePath $Executable
$taskBuildRoot = Join-Path $taskRepoRoot 'local\bridge-build'
$taskCandidate = Join-Path $taskBuildRoot 'release\pixoo-pet.exe'
if ($taskExePath -eq $taskCandidate) { throw 'The installed executable must differ from the build candidate.' }
if (-not $PendingConfig) {
    $PendingConfig = Join-Path (Split-Path -Parent $taskConfigPath) `
        ([IO.Path]::GetFileNameWithoutExtension($taskConfigPath) + '.idle-alternates.pending.toml')
}
$taskPendingPath = Resolve-BridgePath $PendingConfig
$taskHasPending = Test-Path -LiteralPath $taskPendingPath -PathType Leaf
$taskPlanPath = $taskPendingPath + '.plan.json'
$taskConfigHash = Get-PixooSha256 $taskConfigPath
$taskEffectiveConfig = $taskConfigPath
$taskPendingHash = $null

function Assert-BridgeInputs {
    if ((Get-PixooSha256 $taskConfigPath) -ne $taskConfigHash) {
        throw 'The active config changed during preparation. Rerun after reviewing the edit.'
    }
    if ($taskHasPending) {
        if ((Split-Path -Parent $taskPendingPath) -ne (Split-Path -Parent $taskConfigPath)) {
            throw 'The pending config must be beside the active config so relative paths keep their meaning.'
        }
        $plan = Get-Content -LiteralPath $taskPlanPath -Raw | ConvertFrom-Json
        if ($plan.version -ne 1 -or $plan.target_config -ne $taskConfigPath -or
            $plan.source_sha256 -ne $taskConfigHash -or
            $plan.pending_sha256 -ne (Get-PixooSha256 $taskPendingPath)) {
            throw 'The staged config or its source changed. Review and restage the update before installing it.'
        }
    }
}

Assert-BridgeInputs
if ($taskHasPending) {
    $taskEffectiveConfig = $taskPendingPath
    $taskPendingHash = Get-PixooSha256 $taskPendingPath
}
New-Item -ItemType Directory -Path (Join-Path $taskRepoRoot 'local') -Force | Out-Null
$taskLock = [IO.File]::Open((Join-Path $taskRepoRoot 'local\bridge-rebuild.lock'),
    [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
Push-Location -LiteralPath $taskRepoRoot
try {
    if (-not $SkipTests) {
        & cargo test --locked --bin pixoo-pet
        if ($LASTEXITCODE -ne 0) { throw 'Bridge tests failed. The installed bridge has not been touched.' }
        & cargo clippy --locked --bin pixoo-pet --tests -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Bridge Clippy checks failed. The installed bridge has not been touched.' }
        & (Join-Path $taskRepoRoot 'tests\rebuild-bridge.ps1')
        if (-not $?) { throw 'PowerShell bridge guards failed. The installed bridge has not been touched.' }
    }
    # A separate target directory allows builds while the release exe is running.
    & cargo build --locked --release --bin pixoo-pet --target-dir $taskBuildRoot
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed. The installed bridge has not been touched.' }
    $taskCandidateHash = Get-PixooSha256 $taskCandidate
    $taskReportText = & $taskCandidate check-config --config $taskEffectiveConfig
    if ($LASTEXITCODE -ne 0) { throw 'Config/pack validation failed. The installed bridge has not been touched.' }
    $taskReport = $taskReportText -join "`n" | ConvertFrom-Json
    Write-Output "Validated pack $($taskReport.pack); alternates: $($taskReport.idle_alternates -join ', ')."
    if ($CheckOnly) {
        Write-Output 'Checks complete. No bridge was stopped, installed, or started.'
        return
    }

    Assert-BridgeInputs
    if ($taskHasPending -and (Get-PixooSha256 $taskPendingPath) -ne $taskPendingHash) {
        throw 'The pending config changed during the build. Review and rerun.'
    }
    $taskOldProcesses = @()
    $taskExeName = [IO.Path]::GetFileName($taskExePath)
    if ($taskExeName -notmatch '^[A-Za-z0-9._-]+$') { throw 'Unsupported executable filename.' }
    foreach ($item in @(Get-CimInstance Win32_Process -Filter "Name = '$taskExeName'")) {
        if (-not $item.ExecutablePath -or -not $item.CommandLine) {
            throw 'Cannot inspect a bridge process. Run this script as its owning Windows user.'
        }
        if ($item.ExecutablePath -ne $taskExePath -or
            $item.CommandLine -notmatch '^\s*(?:"[^"]+"|\S+)\s+run(?:\s|$)') { continue }
        if (-not (Test-BridgeCommand $item.CommandLine $taskConfigPath)) {
            throw 'This executable is running with another or relative config path. Use -Config for that bridge, or stop it before replacing the shared executable.'
        }
        $taskOldProcesses += $item
    }
    if ($taskOldProcesses.Count -gt 1) { throw 'Multiple matching bridge processes found; resolve them before restarting.' }
    $taskOldDryRun = $taskOldProcesses.Count -eq 1 -and
        $taskOldProcesses[0].CommandLine -match '(?:^|\s)--dry-run(?:\s|$)'
    $taskExistingStatus = Get-BridgeStatus $taskCandidate $taskReport.pipe
    if ($taskExistingStatus -and $taskOldProcesses.Count -eq 0) {
        throw 'The configured pipe is occupied by another bridge. Its process has not been touched.'
    }

    $taskBackupRoot = Join-Path $taskRepoRoot ('local\bridge-restarts\' +
        (Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $taskBackupRoot -Force | Out-Null
    $taskConfigBackup = Join-Path $taskBackupRoot 'bridge.previous.toml'
    $taskExeBackup = Join-Path $taskBackupRoot 'pixoo-pet.previous.exe'
    Copy-Item -LiteralPath $taskConfigPath -Destination $taskConfigBackup
    $taskHadExe = Test-Path -LiteralPath $taskExePath -PathType Leaf
    if ($taskHadExe) { Copy-Item -LiteralPath $taskExePath -Destination $taskExeBackup }
    $taskOldStopped = $false
    $taskNewProcess = $null
    $taskExeReplaced = $false
    $taskConfigReplaced = $false
    $taskLog = Join-Path $taskBackupRoot 'bridge.log'
    try {
        foreach ($item in $taskOldProcesses) {
            $current = Get-CimInstance Win32_Process -Filter "ProcessId = $($item.ProcessId)"
            if (-not $current -or $current.CreationDate -ne $item.CreationDate -or
                $current.ExecutablePath -ne $taskExePath -or
                -not (Test-BridgeCommand $current.CommandLine $taskConfigPath)) {
                throw 'The bridge process changed during preparation. Rerun before stopping anything.'
            }
            Stop-Process -Id $item.ProcessId -ErrorAction Stop
            $taskOldStopped = $true
            Wait-Process -Id $item.ProcessId -Timeout 5 -ErrorAction SilentlyContinue
        }
        New-Item -ItemType Directory -Path (Split-Path -Parent $taskExePath) -Force | Out-Null
        Assert-BridgeInputs
        if ((Get-PixooSha256 $taskCandidate) -ne $taskCandidateHash) { throw 'The build candidate changed before installation.' }
        if (-not $taskHadExe -or (Get-PixooSha256 $taskExePath) -ne $taskCandidateHash) {
            $taskExeReplaced = $true
            Copy-Item -LiteralPath $taskCandidate -Destination $taskExePath -Force
        }
        # Install the compatible binary before exposing new TOML fields to notify.
        if ($taskHasPending) {
            $taskConfigReplaced = $true
            Copy-Item -LiteralPath $taskPendingPath -Destination $taskConfigPath -Force
        }
        $taskNewProcess = Start-BridgeProcess $taskExePath $taskConfigPath $taskLog $taskOldDryRun
        $taskReady = $false
        $taskStartupEnd = [DateTime]::UtcNow.AddSeconds(30)
        while ([DateTime]::UtcNow -lt $taskStartupEnd) {
            $taskNewProcess.Refresh()
            if ($taskNewProcess.HasExited) { throw "Bridge exited during startup. Inspect $taskLog" }
            $state = Get-BridgeStatus $taskExePath $taskReport.pipe
            if ($state -and $state.process_id -eq $taskNewProcess.Id) { $taskReady = $true; break }
            Start-Sleep -Milliseconds 200
        }
        if (-not $taskReady) { throw "Bridge did not become ready. Inspect $taskLog" }
    }
    catch {
        $taskInstallError = $_
        try {
            if ($taskNewProcess) {
                $taskNewProcess.Refresh()
                if (-not $taskNewProcess.HasExited) { $taskNewProcess.Kill(); $taskNewProcess.WaitForExit(5000) | Out-Null }
            }
            if ($taskConfigReplaced) { Copy-Item -LiteralPath $taskConfigBackup -Destination $taskConfigPath -Force }
            if ($taskExeReplaced -and $taskHadExe) { Copy-Item -LiteralPath $taskExeBackup -Destination $taskExePath -Force }
            if ($taskOldStopped) {
                $restored = Start-BridgeProcess $taskExePath $taskConfigPath (Join-Path $taskBackupRoot 'rollback.log') $taskOldDryRun
                Write-Warning "Restored the previous bridge as process $($restored.Id). Backups: $taskBackupRoot"
            }
        }
        catch { Write-Warning "Rollback needs attention: $($_.Exception.Message). Backups: $taskBackupRoot" }
        throw $taskInstallError
    }

    if ($taskHasPending) {
        # Archive only these two exact files. Future runs use the installed config.
        try {
            Move-Item -LiteralPath $taskPendingPath -Destination (Join-Path $taskBackupRoot 'installed.pending.toml')
            Move-Item -LiteralPath $taskPlanPath -Destination (Join-Path $taskBackupRoot 'installed.plan.json')
        }
        catch { Write-Warning "Bridge is running, but staging-file archival failed: $($_.Exception.Message)" }
    }
    Write-Output "Started bridge process $($taskNewProcess.Id) on $($taskReport.pipe)."
    Write-Output "Log: $taskLog"
    Write-Output "Backups: $taskBackupRoot"
    Write-Output 'New GIFs preload at startup; unchanged files reuse existing transfer receipts.'
}
finally {
    Pop-Location
    $taskLock.Dispose()
}
