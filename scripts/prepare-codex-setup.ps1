param(
    [string]$Config = 'local\bridge.toml',
    [string]$Executable,
    [string]$CodexHome,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent $PSScriptRoot
function Resolve-WorkspaceFile([string]$Path) {
    $candidate = if ([IO.Path]::IsPathRooted($Path)) { $Path } else { Join-Path $repoRoot $Path }
    return (Resolve-Path -LiteralPath $candidate).Path
}
if (-not $Executable) {
    $Executable = if (Test-Path -LiteralPath (Join-Path $repoRoot 'pixoo-pet.exe')) { 'pixoo-pet.exe' } else { 'target\release\pixoo-pet.exe' }
}
$exePath = Resolve-WorkspaceFile $Executable
$configPath = Resolve-WorkspaceFile $Config
if (-not $OutputDirectory) { $OutputDirectory = 'local\codex-setup-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 6) }
$outputPath = if ([IO.Path]::IsPathRooted($OutputDirectory)) { $OutputDirectory } else { Join-Path $repoRoot $OutputDirectory }
$outputPath = [IO.Path]::GetFullPath($outputPath)
$arguments = @('setup', '--config', $configPath, '--output', $outputPath)
if ($CodexHome) { $arguments += @('--codex-home', $CodexHome) }
& $exePath @arguments
if ($LASTEXITCODE -ne 0) { throw 'Setup preparation failed; destination settings were not modified.' }
$metadata = Get-Content -LiteralPath (Join-Path $outputPath 'metadata.json') -Raw | ConvertFrom-Json
$files = @()
foreach ($item in $metadata.files) {
    $originalHash = $null
    if ($item.existed) {
        $originalHash = Get-PixooSha256 (Join-Path $outputPath $item.original)
        if (-not (Test-Path -LiteralPath $item.destination -PathType Leaf) -or
            (Get-PixooSha256 $item.destination) -ne $originalHash) {
            throw "Settings changed during preparation: $($item.destination). Prepare a new plan."
        }
    } elseif (Test-Path -LiteralPath $item.destination) {
        throw "Settings appeared during preparation: $($item.destination). Prepare a new plan."
    }
    $files += [ordered]@{
        source = $item.source
        destination = $item.destination
        original_sha256 = $originalHash
        proposed_sha256 = Get-PixooSha256 (Join-Path $outputPath $item.source)
    }
}
[ordered]@{version=1; files=$files} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputPath 'plan.json') -Encoding UTF8
Write-Output "Review the three proposed files in $outputPath. Existing settings snapshots are private; keep this directory out of source control."
Write-Output "Apply after review: & '$PSScriptRoot\apply-codex-setup.ps1' -PlanDirectory '$outputPath' -Apply"
Write-Output 'Then review/trust the new or changed hooks in Codex. This command has not changed the global settings or bridge config.'
