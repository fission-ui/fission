!ifndef FISSION_SHORTCUT_AUMID_NSH
!define FISSION_SHORTCUT_AUMID_NSH

!include "LogicLib.nsh"

; Embed the architecture-matched helper once in an installer section.
!macro FissionEmbedShortcutAppUserModelIdHelper HELPER_PATH
  InitPluginsDir
  File "/oname=$PLUGINSDIR\fission-shortcut-aumid.exe" "${HELPER_PATH}"
!macroend

; Apply the same stable AppUserModelID passed to
; WinitApp::with_windows_app_user_model_id or
; DesktopApp::with_windows_app_user_model_id. Call this after CreateShortCut.
!macro FissionSetShortcutAppUserModelId SHORTCUT_PATH APP_USER_MODEL_ID
  Push $0
  Push $1
  nsExec::ExecToStack /TIMEOUT=30000 '"$PLUGINSDIR\fission-shortcut-aumid.exe" "${SHORTCUT_PATH}" "${APP_USER_MODEL_ID}"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "Failed to apply AppUserModelID to ${SHORTCUT_PATH}: exit=$0 output=$1"
    MessageBox MB_ICONSTOP|MB_OK "Windows notification identity setup failed. The installation cannot continue."
    Pop $1
    Pop $0
    SetErrors
    Abort
  ${EndIf}
  Pop $1
  Pop $0
!macroend

!endif
