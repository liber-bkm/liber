; NSIS installer hooks for the liber desktop app (Tauri v2).
; Referenced from tauri.conf.json as bundle.windows.nsis.installerHooks.
; Runs inside the upstream installer.nsi at the POSTINSTALL and
; POSTUNINSTALL hook points, so a Tauri upgrade only breaks this if
; those four hook names change (CI builds the installer on every tag).
;
; What it does: appends $INSTDIR to the *user* PATH on install so
; `liber` and `liber-serve` work from any terminal, and removes exactly
; that entry on uninstall. HKCU (not HKLM) on purpose: no admin
; escalation, and per-user PATH is where a terminal resolves it.

!include "StrFunc.nsh"
!include "WinMessages.nsh"

${Using:StrFunc} StrStr
${Using:StrFunc} StrRep
${Using:StrFunc} UnStrRep

!macro NSIS_HOOK_POSTINSTALL
  ReadRegStr $0 HKCU "Environment" "Path"
  ${StrStr} $1 "$0" "$INSTDIR"
  StrCmp $1 "" 0 +3
    WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ReadRegStr $0 HKCU "Environment" "Path"
  ${UnStrRep} $0 "$0" ";$INSTDIR" ""
  ${UnStrRep} $0 "$0" "$INSTDIR;" ""
  ${UnStrRep} $0 "$0" "$INSTDIR" ""
  WriteRegExpandStr HKCU "Environment" "Path" "$0"
  SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend
