Unicode true

!ifndef APP_VERSION
  !error "APP_VERSION is required"
!endif
!ifndef NUMERIC_VERSION
  !error "NUMERIC_VERSION is required"
!endif
!ifndef SOURCE_DIR
  !error "SOURCE_DIR is required"
!endif
!ifndef OUTPUT_FILE
  !error "OUTPUT_FILE is required"
!endif
!ifndef ICON_FILE
  !error "ICON_FILE is required"
!endif

Name "ToskLight Pixel"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\ToskLight Pixel"
InstallDirRegKey HKCU "Software\ToskLight\Pixel" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "${ICON_FILE}"
UninstallIcon "${ICON_FILE}"

VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "ToskLight Pixel"
VIAddVersionKey /LANG=1033 "FileDescription" "ToskLight Pixel installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "OriginalFilename" "ToskLight Pixel Setup.exe"

Page directory
Page instfiles

Section "ToskLight Pixel" SEC_PIXEL
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File "/oname=ToskLight Pixel.exe" "${SOURCE_DIR}\ToskLight Pixel.exe"
  File /oname=media-server.exe "${SOURCE_DIR}\media-server.exe"
  WriteUninstaller "$INSTDIR\Uninstall ToskLight Pixel.exe"
  WriteRegStr HKCU "Software\ToskLight\Pixel" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ToskLight Pixel" "DisplayName" "ToskLight Pixel"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ToskLight Pixel" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ToskLight Pixel" "DisplayIcon" "$INSTDIR\ToskLight Pixel.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ToskLight Pixel" "UninstallString" '"$INSTDIR\Uninstall ToskLight Pixel.exe"'
  CreateDirectory "$SMPROGRAMS\ToskLight"
  CreateShortcut "$SMPROGRAMS\ToskLight\ToskLight Pixel.lnk" "$INSTDIR\ToskLight Pixel.exe" "" "$INSTDIR\ToskLight Pixel.exe"
  CreateShortcut "$DESKTOP\ToskLight Pixel.lnk" "$INSTDIR\ToskLight Pixel.exe" "" "$INSTDIR\ToskLight Pixel.exe"
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$DESKTOP\ToskLight Pixel.lnk"
  Delete "$SMPROGRAMS\ToskLight\ToskLight Pixel.lnk"
  RMDir "$SMPROGRAMS\ToskLight"
  Delete "$INSTDIR\ToskLight Pixel.exe"
  Delete "$INSTDIR\media-server.exe"
  Delete "$INSTDIR\Uninstall ToskLight Pixel.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ToskLight Pixel"
  DeleteRegKey HKCU "Software\ToskLight\Pixel"
SectionEnd
