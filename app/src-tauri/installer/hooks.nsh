; RestedRealm Companion installer hooks (Tauri NSIS template).

!macro NSIS_HOOK_PREUNINSTALL
  ; The app starts itself with Windows through this value; remove it with the app.
  ; Installing a newer version runs this uninstaller with /UPDATE: keep it then.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "RestedRealm Companion"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "RestedRealm Companion"
  ${EndIf}
  ; "Delete the application data" also forgets this PC's RestedRealm connection,
  ; which lives in Windows Credential Manager rather than in a folder.
  ${If} $DeleteAppDataCheckboxState = 1
    ExecWait '"$INSTDIR\RestedRealm Companion.exe" --uninstall-cleanup'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
    RMDir /r "$LOCALAPPDATA\RestedRealm Companion"
  ${EndIf}
!macroend
