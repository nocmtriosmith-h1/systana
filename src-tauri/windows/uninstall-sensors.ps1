# Run elevated by the installer before files are replaced or removed (see hooks.nsh).
# Stops the sensor helper so systana.exe is not locked. With -Remove, also deletes the task and its data.
param([switch]$Remove)
$ErrorActionPreference = "SilentlyContinue"

$taskName = "Systana Sensors"
Stop-ScheduledTask -TaskName $taskName
# The helper is the only systana.exe running as SYSTEM.
Get-CimInstance Win32_Process -Filter "Name = 'systana.exe'" |
    Where-Object { $_.CommandLine -match "--sensors" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force }

if ($Remove) {
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
    # rmdir /s never follows a junction, so a planted link cannot redirect the delete.
    $data = Join-Path $env:ProgramData "Systana"
    if (Test-Path -LiteralPath $data) { & cmd.exe /d /c rmdir /s /q $data }
}
