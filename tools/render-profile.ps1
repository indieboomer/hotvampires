param([string]$Executable='target/iteration/debug/hot-vampires.exe',[ValidateSet('Walk','Rotate','RuinFall')][string]$Scenario='Walk')
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $project
$started = Get-Date
$profileArguments = @('--render-profile','--frames','240')
if ($Scenario -eq 'RuinFall') { $profileArguments += @('--ruin-demo','--rotate-profile') } else { $profileArguments += '--terrain-walk'; if ($Scenario -eq 'Rotate') { $profileArguments += '--rotate-profile' } }
$process = Start-Process -FilePath "$project/$Executable" -ArgumentList $profileArguments -WorkingDirectory $project -WindowStyle Hidden -PassThru -RedirectStandardOutput "$project/target/profile-stdout.txt" -RedirectStandardError "$project/target/profile-stderr.txt"
if (-not $process.WaitForExit(45000)) { Stop-Process -Id $process.Id; throw 'Render profile timed out' }
if (-not (Test-Path -LiteralPath target/native-render-profile.json) -or (Get-Item -LiteralPath target/native-render-profile.json).LastWriteTime -lt $started) { Get-Content -LiteralPath target/profile-stderr.txt; throw 'Render profile failed to produce a new report' }
Get-Content -LiteralPath target/profile-stdout.txt
Get-Content -LiteralPath target/profile-stderr.txt
