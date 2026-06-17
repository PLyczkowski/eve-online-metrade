@echo off
setlocal
cd /d "%~dp0"
set "APP=%~dp0src-tauri\target\release\eve-metrade.exe"
set "CARGO_BIN=%USERPROFILE%\.cargo\bin"

if exist "%CARGO_BIN%\cargo.exe" (
  set "PATH=%CARGO_BIN%;%PATH%"
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo Rust/Cargo was not found.
  echo Install Rust, then run this launcher again:
  echo   https://rustup.rs/
  pause
  exit /b 1
)

echo Building latest EVE Metrade...
call npm run tauri build
if errorlevel 1 (
  echo.
  echo Build failed. The app was not launched.
  pause
  exit /b 1
)

start "" "%APP%"
exit /b 0
