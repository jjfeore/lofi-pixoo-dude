# Run in the user's PowerShell when the Codex sandbox blocks directory metadata.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $taskRepoRoot
try {
    & cargo clippy --locked --release --example work_revision -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed; review its output above.' }

    & (Join-Path $taskRepoRoot 'target/release/pixoo-pet.exe') validate (Join-Path $taskRepoRoot 'pets/decker/revisions/work-v13/review')
    if ($LASTEXITCODE -ne 0) { throw 'Pack validation failed; review its output above.' }

    Write-Output 'Motorized artwork static checks and pack validation passed.'
}
finally {
    Pop-Location
}
