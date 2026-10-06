# Run in the user's PowerShell when the Codex sandbox blocks directory metadata.
[CmdletBinding()]
param(
    [string]$Config = 'local\bridge.toml',
    [string]$Python,
    [switch]$Format,
    [switch]$Sync,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
$taskConfigPath = if ([IO.Path]::IsPathRooted($Config)) { $Config } else { Join-Path $taskRepoRoot $Config }
$taskExePath = Join-Path $taskRepoRoot 'target\release\pixoo-pet.exe'
if (-not $Python) {
    $taskBundledPython = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
    $Python = if (Test-Path -LiteralPath $taskBundledPython) { $taskBundledPython } else { 'python' }
}

Push-Location -LiteralPath $taskRepoRoot
try {
    if ($Format) {
        & rustfmt --edition 2024 --config skip_children=true src/config.rs src/device.rs src/main.rs src/runtime.rs src/storage.rs
        if ($LASTEXITCODE -ne 0) { throw 'Bridge formatting failed.' }
    }
    & cargo test --locked --bin pixoo-pet
    if ($LASTEXITCODE -ne 0) { throw 'Bridge tests failed.' }
    & cargo clippy --locked --bin pixoo-pet --tests -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Bridge Clippy checks failed.' }
    & cargo build --locked --release --bin pixoo-pet
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    & $Python -X utf8 tests/integration.py -v
    if ($LASTEXITCODE -ne 0) { throw 'CLI/named-pipe integration failed.' }
    & $taskExePath sync-gifs --config $taskConfigPath --prepare-only
    if ($LASTEXITCODE -ne 0) { throw 'Configured pack preparation failed.' }
    if ($Sync) {
        $taskSyncArguments = @('sync-gifs', '--config', $taskConfigPath)
        if ($Force) { $taskSyncArguments += '--force' }
        & $taskExePath @taskSyncArguments
        if ($LASTEXITCODE -ne 0) { throw 'Device preload failed; review the download diagnostic above.' }
    }
    Write-Output 'Stored-GIF checks completed. Device playback still needs visual verification.'
}
finally {
    Pop-Location
}
