!macro NSIS_HOOK_PREINSTALL
  ; Stop the desktop first so it cannot restart a reused Light server while files are replaced.
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /T /IM ToskLight.exe'
  Pop $0
  nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /IM light-headless.exe'
  Pop $0
  Sleep 1000
!macroend
