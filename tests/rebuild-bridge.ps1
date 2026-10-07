# Read the script's guard functions without executing its build/install body.
$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $taskRepoRoot 'scripts\common.ps1')
$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile(
    (Join-Path $taskRepoRoot 'scripts\rebuild-bridge.ps1'), [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw ($errors -join "`n") }
foreach ($name in @('Test-BridgeCommand', 'Assert-BridgeInputs', 'Start-BridgeProcess')) {
    $node = $ast.Find({ param($item)
        $item -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $item.Name -eq $name
    }, $true)
    if (-not $node) { throw "Missing guard: $name" }
    . ([scriptblock]::Create($node.Extent.Text))
}

$plainConfig = 'C:\repo\local\bridge.toml'
$spacedConfig = 'C:\my repo\local\bridge.toml'
$cases = @(
    @('C:\repo\pixoo-pet.exe run -c C:\repo\local\bridge.toml', $plainConfig, $true),
    @('"C:\my repo\pixoo-pet.exe" run --config "C:\my repo\local\bridge.toml"', $spacedConfig, $true),
    @('C:\repo\pixoo-pet.exe run --config="C:\repo\local\bridge.toml" --dry-run', $plainConfig, $true),
    @('C:\repo\pixoo-pet.exe run --config C:\repo\local\bridge.toml.bak', $plainConfig, $false),
    @('C:\repo\pixoo-pet.exe run --config "C:\repo\local\bridge.toml"extra', $plainConfig, $false),
    @('C:\repo\pixoo-pet.exe notify -c C:\repo\local\bridge.toml', $plainConfig, $false),
    @('C:\repo\pixoo-pet.exe emit --pipe live', $plainConfig, $false),
    @('C:\repo\pixoo-pet.exe run --config local\bridge.toml', $plainConfig, $false),
    @('C:\repo\pixoo-pet.exe run --config C:\my repo\local\bridge.toml', $spacedConfig, $false),
    @('C:\repo\pixoo-pet.exe run --config C:\repo\other.toml', $plainConfig, $false)
)
foreach ($case in $cases) {
    if ((Test-BridgeCommand $case[0] $case[1]) -ne $case[2]) { throw "Process match failed: $($case[0])" }
}

function Start-Process {
    param($FilePath, $ArgumentList, $WorkingDirectory, $WindowStyle,
        [switch]$PassThru, $RedirectStandardError, $RedirectStandardOutput)
    return [pscustomobject]@{Tool=$FilePath; Arguments=$ArgumentList; WindowStyle=$WindowStyle;
        ErrorLog=$RedirectStandardError; OutputLog=$RedirectStandardOutput}
}
$launch = Start-BridgeProcess 'C:\repo\pixoo-pet.exe' $spacedConfig 'C:\repo\test.log' $true
if ($launch.WindowStyle -ne 'Hidden' -or $launch.Arguments -notcontains '--dry-run' -or
    $launch.Arguments[2] -ne ('"{0}"' -f $spacedConfig) -or $launch.OutputLog -ne 'C:\repo\test.log.stdout') {
    throw 'Hidden launch arguments or dry-run preservation failed.'
}
$launch = Start-BridgeProcess 'C:\repo\pixoo-pet.exe' $plainConfig 'C:\repo\test.log'
if ($launch.Arguments -contains '--dry-run') { throw 'Live launch unexpectedly became a dry run.' }

$testRoot = Join-Path $taskRepoRoot ('local\bridge-script-tests\' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$taskConfigPath = Join-Path $testRoot 'bridge.toml'
$taskPendingPath = Join-Path $testRoot 'bridge.idle-alternates.pending.toml'
$taskPlanPath = $taskPendingPath + '.plan.json'
$taskHasPending = $true
[IO.File]::WriteAllText($taskConfigPath, 'original')
[IO.File]::WriteAllText($taskPendingPath, 'updated')
$taskConfigHash = Get-PixooSha256 $taskConfigPath
$plan = @{version=1; target_config=$taskConfigPath; source_sha256=$taskConfigHash;
    pending_sha256=(Get-PixooSha256 $taskPendingPath)}
[IO.File]::WriteAllText($taskPlanPath, ($plan | ConvertTo-Json))
Assert-BridgeInputs

function Assert-Rejected([scriptblock]$Action) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'Unsafe staging inputs were accepted.' }
}
[IO.File]::WriteAllText($taskConfigPath, 'intervening config edit')
Assert-Rejected { Assert-BridgeInputs }
[IO.File]::WriteAllText($taskConfigPath, 'original')
[IO.File]::WriteAllText($taskPendingPath, 'intervening staged edit')
Assert-Rejected { Assert-BridgeInputs }
[IO.File]::WriteAllText($taskPendingPath, 'updated')
$plan.target_config = Join-Path $testRoot 'other.toml'
[IO.File]::WriteAllText($taskPlanPath, ($plan | ConvertTo-Json))
Assert-Rejected { Assert-BridgeInputs }
$taskHasPending = $false
Assert-BridgeInputs
Write-Output 'PASS: PowerShell syntax, 10 process matches, hidden/dry-run launch arguments, and staging/edit guards. No live processes were touched.'
