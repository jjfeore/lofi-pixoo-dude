param([string]$PlanDirectory = 'local\codex-install', [switch]$Apply)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent $PSScriptRoot
$planPath = if ([IO.Path]::IsPathRooted($PlanDirectory)) { $PlanDirectory } else { Join-Path $repoRoot $PlanDirectory }
$planPath = (Resolve-Path -LiteralPath $planPath).Path
$plan = Get-Content -LiteralPath (Join-Path $planPath 'plan.json') -Raw | ConvertFrom-Json
foreach ($item in $plan.files) {
    if ($item.original_sha256) {
        if (-not (Test-Path -LiteralPath $item.destination -PathType Leaf)) { throw "Expected existing file missing: $($item.destination)" }
        $actual = Get-PixooSha256 $item.destination
        if ($actual -ne $item.original_sha256) { throw "Settings changed since this plan was prepared: $($item.destination). Regenerate the plan before applying it." }
    } elseif (Test-Path -LiteralPath $item.destination) { throw "Destination appeared since preparation: $($item.destination). Regenerate the plan before applying it." }
    $source = Join-Path $planPath $item.source
    if ((Get-PixooSha256 $source) -ne $item.proposed_sha256) { throw "Proposed file changed: $source. Review and regenerate the plan." }
}
if (-not $Apply) {
    Write-Output 'Plan checks pass. Review the proposed files, then run this script with -Apply to install them.'
    exit 0
}
$backup = Join-Path $planPath ('backup-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
New-Item -ItemType Directory -Path $backup | Out-Null
foreach ($item in $plan.files) {
    if ($item.original_sha256) { Copy-Item -LiteralPath $item.destination -Destination (Join-Path $backup $item.source) }
}
try {
    foreach ($item in $plan.files) {
        $destinationParent = Split-Path -Parent $item.destination
        if (-not (Test-Path -LiteralPath $destinationParent)) { New-Item -ItemType Directory -Path $destinationParent | Out-Null }
        Copy-Item -LiteralPath (Join-Path $planPath $item.source) -Destination $item.destination
    }
} catch {
    foreach ($item in $plan.files) {
        if ($item.original_sha256) { Copy-Item -LiteralPath (Join-Path $backup $item.source) -Destination $item.destination }
        elseif (Test-Path -LiteralPath $item.destination -PathType Leaf) { Remove-Item -LiteralPath $item.destination }
    }
    throw
}
Write-Output "Installed hook forwarding; originals saved in $backup."
Write-Output 'Restart Codex if needed, then review/trust the new Pixoo hook commands in Codex. Existing hook trust entries were preserved.'
