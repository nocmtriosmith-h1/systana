# Run elevated by the installer (see hooks.nsh). Sets up temperature reading:
#  1. installs the signed PawnIO driver if it is not already present,
#  2. creates %ProgramData%\Systana, writable only by SYSTEM/Administrators,
#  3. registers the "Systana Sensors" task that runs `systana.exe --sensors` as SYSTEM at boot.
param([Parameter(Mandatory = $true)][string]$InstallDir)
$ErrorActionPreference = "Continue"

$taskName = "Systana Sensors"
$exe = Join-Path $InstallDir "systana.exe"
# The NSIS installer is 32-bit; if this script ever runs under WOW64, $env:ProgramFiles is
# "Program Files (x86)". ProgramW6432 always names the 64-bit Program Files.
$pf64 = if ($env:ProgramW6432) { $env:ProgramW6432 } else { $env:ProgramFiles }

# The task runs as SYSTEM, so its exe must live where ordinary users cannot replace it
# (or rename its folder). Only Program Files guarantees that; any other location gets no sensors.
$programFiles = [IO.Path]::GetFullPath($pf64).TrimEnd('\') + '\'
$fullInstall = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\') + '\'
if (-not $fullInstall.StartsWith($programFiles, [StringComparison]::OrdinalIgnoreCase)) {
    Write-Output "Systana is not installed under $programFiles; temperature sensors are disabled for safety."
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
    exit 0
}

# 1. PawnIO driver (shared with other hardware tools, so it is kept on uninstall).
$pawnio = Join-Path $pf64 "PawnIO\PawnIOLib.dll"
if (-not (Test-Path $pawnio)) {
    $setup = Join-Path $InstallDir "PawnIO_setup.exe"
    if (Test-Path $setup) {
        Start-Process -FilePath $setup -ArgumentList "-install", "-silent" -Wait
    }
}

# 2. The helper runs as SYSTEM and writes here, so ordinary users must not be able to
#    plant files or links in this folder. Inheritance is removed and Users get read-only.
#    Any user may create folders in ProgramData, so a folder that already exists may have been
#    planted. A junction is removed without being followed, the owner is reset to Administrators
#    (so the planter keeps no implicit right to change the permissions back), and everything
#    inside is deleted once the folder is locked.

# Deletes a file, folder or link without ever following a link (rmdir /s does not traverse junctions).
function Remove-NoFollow([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return }
    if ($item.PSIsContainer) { & cmd.exe /d /c rmdir /s /q $Path 2>$null }
    else { Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue }
}

$data = Join-Path $env:ProgramData "Systana"
$existing = Get-Item -LiteralPath $data -Force -ErrorAction SilentlyContinue
if ($existing -and ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint)) { Remove-NoFollow $data }
New-Item -ItemType Directory -Force $data | Out-Null
& icacls $data /setowner "*S-1-5-32-544" | Out-Null
& icacls $data /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" "*S-1-5-32-545:(OI)(CI)RX" | Out-Null
Get-ChildItem -LiteralPath $data -Force | ForEach-Object { Remove-NoFollow $_.FullName }

# Sysrewind was this app's old name: remove its helper task and data so two helpers never run.
Unregister-ScheduledTask -TaskName "Sysrewind Sensors" -Confirm:$false -ErrorAction SilentlyContinue
Remove-NoFollow (Join-Path $env:ProgramData "Sysrewind")

# 3. Scheduled task: start at boot, never time out, keep running on battery, restart if it crashes.
$action = New-ScheduledTaskAction -Execute $exe -Argument "--sensors"
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -MultipleInstances IgnoreNew
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal `
    -Settings $settings -Description "Reads CPU and disk temperatures for Systana." -Force | Out-Null
Start-ScheduledTask -TaskName $taskName
