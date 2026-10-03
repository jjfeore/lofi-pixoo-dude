param([string]$Executable = 'target\release\pixoo-pet.exe')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')
$repoRoot = Split-Path -Parent $PSScriptRoot
$exePath = if ([IO.Path]::IsPathRooted($Executable)) { $Executable } else { Join-Path $repoRoot $Executable }
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) { throw 'Build the release executable first: cargo build --locked --release' }
$version = '0.1.0'
$bundle = Join-Path $repoRoot "dist\pixoo-pet-$version-windows-x64"
if (Test-Path -LiteralPath $bundle) { throw 'Bundle directory exists; choose a clean output before packaging.' }
New-Item -ItemType Directory -Path $bundle | Out-Null
Copy-Item -LiteralPath $exePath -Destination (Join-Path $bundle 'pixoo-pet.exe')
foreach ($name in @('README.md', 'docs', 'config', 'pets', 'plugin', 'scripts')) {
    Copy-Item -LiteralPath (Join-Path $repoRoot $name) -Destination $bundle -Recurse
}
$archive = "$bundle.zip"
Compress-Archive -LiteralPath $bundle -DestinationPath $archive
$checksum = Get-PixooSha256 $archive
Set-Content -LiteralPath "$archive.sha256" -Value "$checksum  $([IO.Path]::GetFileName($archive))" -Encoding ascii
Write-Output $archive
