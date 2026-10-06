# Run outside the Codex sandbox when Rust tools cannot read directory metadata.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('idle_revision', 'work_revision', 'compaction_revision', 'interruption_revision', 'delegation_revision', 'atlas_cells')]
    [string]$Example,
    [Parameter(Mandatory = $true)]
    [string[]]$Pack,
    [switch]$Format
)

$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $taskRepoRoot
try {
    if ($Format) {
        & rustfmt --edition 2024 "examples/$Example.rs"
        if ($LASTEXITCODE -ne 0) { throw 'Artwork helper formatting failed.' }
    }

    & cargo clippy --locked --release --example $Example -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed; review its output above.' }

    if ($Example -eq 'delegation_revision') {
        if ($Format) {
            & rustfmt --edition 2024 src/assets.rs src/engine.rs src/runtime.rs
            if ($LASTEXITCODE -ne 0) { throw 'Delegation runtime formatting failed.' }
        }
        & cargo test --locked --release
        if ($LASTEXITCODE -ne 0) { throw 'Runtime tests failed; review their output above.' }
        & cargo clippy --locked --release --bin pixoo-pet -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Runtime Clippy failed; review its output above.' }
        & cargo build --locked --release --bin pixoo-pet
        if ($LASTEXITCODE -ne 0) { throw 'Updated runtime build failed.' }
    }

    foreach ($taskPackPath in $Pack) {
        & (Join-Path $taskRepoRoot 'target/release/pixoo-pet.exe') validate $taskPackPath
        if ($LASTEXITCODE -ne 0) { throw "Pack validation failed: $taskPackPath" }
    }
    Write-Output 'Artwork static checks and pack validation passed.'
}
finally {
    Pop-Location
}
