@echo off
:: Load VS Build Tools environment
call "D:\Microsoft\VisualStudio\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (
    echo Failed to load VS environment
    pause
    exit /b 1
)

:: Run tauri dev
cd /d D:\copy\clipboard-manager-tauri
npm run tauri dev
