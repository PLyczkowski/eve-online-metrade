@echo off
set "APP=%~dp0src-tauri\target\release\eve-metrade.exe"

if not exist "%APP%" (
  echo EVE Metrade is not built yet.
  echo Run this first:
  echo   npm run tauri build
  pause
  exit /b 1
)

start "" "%APP%"
