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
:: 按修改时间取最新的安装包,避免版本号升级后此处漏改
for /f "delims=" %%f in ('dir /b /o-d "src-tauri\target\release\bundle\nsis\clipboard_*_x64-setup.exe"') do (
    copy /Y "src-tauri\target\release\bundle\nsis\%%f" clipboard-setup.exe >nul
    goto :copied
)
echo WARNING: NSIS installer not found
:copied
echo === Artifacts ===
echo Installer: clipboard-setup.exe
echo Standalone exe: src-tauri\target\release\clipboard-manager-tauri.exe
echo Layout: config\ (settings) + data\ (clipboard data) next to the exe
