; Version 1.0.9 renamed the app from "SleepyMiles Mod Manager" (SleepyMilesModManager.exe) to "Miles" (Miles.exe).
; Windows treats that as a different program, so these hooks move people over from the old install:
; the old app is closed, its shortcuts are replaced with "Miles" ones, its taskbar pin is pointed at the new
; exe, and the old program folder and its "Apps" entry are removed.
; Profiles, settings and the GitHub login live in %APPDATA%\com.sleepymiles.modmanager and are not touched.

!define OLD_PRODUCTNAME "SleepyMiles Mod Manager"
!define OLD_EXE "SleepyMilesModManager.exe"
!define OLD_INSTDIR "$LOCALAPPDATA\SleepyMiles Mod Manager"
!define OLD_TASKBAR_PIN "$APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\SleepyMiles Mod Manager.lnk"

!macro NSIS_HOOK_PREINSTALL
  ${If} ${FileExists} "${OLD_INSTDIR}\${OLD_EXE}"
    !insertmacro CheckIfAppIsRunning "${OLD_EXE}" "${OLD_PRODUCTNAME}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} ${FileExists} "${OLD_INSTDIR}\${OLD_EXE}"
    ; desktop shortcut: only if they had one before
    ${If} ${FileExists} "$DESKTOP\${OLD_PRODUCTNAME}.lnk"
      Delete "$DESKTOP\${OLD_PRODUCTNAME}.lnk"
      CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}

    ; start menu shortcut
    Delete "$SMPROGRAMS\${OLD_PRODUCTNAME}.lnk"
    ${IfNot} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}

    ; taskbar pin: keep it, but open the new exe (it picks up the new icon)
    ${If} ${FileExists} "${OLD_TASKBAR_PIN}"
      !insertmacro SetShortcutTarget "${OLD_TASKBAR_PIN}" "$INSTDIR\${MAINBINARYNAME}.exe"
    ${EndIf}

    ; the old program folder (only program files are in it) and its "Apps" entry
    RMDir /r "${OLD_INSTDIR}"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${OLD_PRODUCTNAME}"
    DeleteRegKey HKCU "${MANUKEY}\${OLD_PRODUCTNAME}"
  ${EndIf}
!macroend
