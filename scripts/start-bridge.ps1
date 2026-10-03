param([string]$Config = 'local\bridge.toml', [string]$Executable, [switch]$DryRun)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $Executable) {
    $Executable = if (Test-Path -LiteralPath (Join-Path $repoRoot 'pixoo-pet.exe')) { 'pixoo-pet.exe' } else { 'target\release\pixoo-pet.exe' }
}
$exePath = if ([IO.Path]::IsPathRooted($Executable)) { $Executable } else { Join-Path $repoRoot $Executable }
$configPath = if ([IO.Path]::IsPathRooted($Config)) { $Config } else { Join-Path $repoRoot $Config }
$exePath = (Resolve-Path -LiteralPath $exePath).Path
$configPath = (Resolve-Path -LiteralPath $configPath).Path
$logRoot = Join-Path $repoRoot 'local'
New-Item -ItemType Directory -Path $logRoot -Force | Out-Null
$arguments = @('run', '--config', ('"{0}"' -f $configPath))
if ($DryRun) { $arguments += '--dry-run' }
$logPath = Join-Path $logRoot ('bridge-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '.log')
$process = Start-Process -FilePath $exePath -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardError $logPath -RedirectStandardOutput ($logPath + '.stdout')
Start-Sleep -Milliseconds 250
if ($process.HasExited) { throw "Bridge exited during startup; inspect $logPath." }
Write-Output "Started bridge process $($process.Id). Use the configured pipe with pixoo-pet status."
Write-Output "Log: $logPath"
