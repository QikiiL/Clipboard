@echo off
:: One-click release build: load VS build tools env, then run tauri build (NSIS exe installer)
call "D:\Microsoft\VisualStudio\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (
    echo Failed to load VS environment
    pause
    exit /b 1
)

cd /d D:\copy\clipboard-manager-tauri
npm run tauri build
if errorlevel 1 (
    echo.
    echo Build FAILED
    pause
    exit /b 1
)

echo.
copy /Y src-tauri\target\release\bundle\nsis\clipboard_0.1.0_x64-setup.exe clipboard-setup.exe >nul
echo === Artifacts ===
echo Installer: clipboard-setup.exe
echo Standalone exe: src-tauri\target\release\clipboard-manager-tauri.exe
echo Layout: config\ (settings) + data\ (clipboard data) next to the exe
