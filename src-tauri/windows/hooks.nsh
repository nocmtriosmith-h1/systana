; Tauri NSIS hooks: set up and tear down the elevated temperature helper.
; The installer is per-machine, so these run with administrator rights (one UAC prompt).
!include "LogicLib.nsh"

; The installer is a 32-bit process; "sysnative" reaches the 64-bit PowerShell so the
; scripts see the real Program Files and the 64-bit task scheduler / registry views.
; The scripts run elevated from $INSTDIR, so they only run when $INSTDIR is inside Program Files:
; anywhere else an ordinary user might be able to swap a script before it runs.
!macro SYSTANA_PS SCRIPT ARGS
  ; Preserve registers the surrounding Tauri installer code may rely on.
  Push $0
  Push $1
  Push $2
  StrCpy $0 "$PROGRAMFILES64\"
  StrLen $1 $0
  StrCpy $2 "$INSTDIR\" $1
  ${If} $2 == $0 ; NSIS string comparison is case-insensitive
  ${AndIf} ${FileExists} "$INSTDIR\${SCRIPT}"
    StrCpy $1 "$WINDIR\sysnative\WindowsPowerShell\v1.0\powershell.exe"
    ${IfNot} ${FileExists} "$1"
      StrCpy $1 "powershell.exe"
    ${EndIf}
    nsExec::ExecToLog '"$1" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\${SCRIPT}" ${ARGS}'
    Pop $0 ; exit code, unused
  ${EndIf}
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Upgrading: stop the running helper so systana.exe can be replaced.
  !insertmacro SYSTANA_PS "uninstall-sensors.ps1" ""
  ; Systana was called Sysrewind up to v0.1.1. Remove that install quietly (its own hook stops
  ; and deletes its helper task); the user's history is kept and moved over on first start.
  ${If} ${FileExists} "$PROGRAMFILES64\Sysrewind\uninstall.exe"
    DetailPrint "Removing Sysrewind (old name of Systana)..."
    ExecWait '"$PROGRAMFILES64\Sysrewind\uninstall.exe" /S _?=$PROGRAMFILES64\Sysrewind'
    RMDir /r "$PROGRAMFILES64\Sysrewind"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Setting up temperature sensors..."
  !insertmacro SYSTANA_PS "install-sensors.ps1" '-InstallDir "$INSTDIR"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro SYSTANA_PS "uninstall-sensors.ps1" "-Remove"
!macroend
