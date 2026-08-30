@echo off
setlocal

rem =============================================================
rem  Clipboard Manager - dev runner (GNU toolchain, elevated)
rem
rem  Why this script exists:
rem   - Elevation: Win+V takeover and pasting into elevated
rem     windows require the same integrity level, so the app is
rem     meant to run as administrator.
rem   - GNU toolchain: rustup default is already
rem     stable-x86_64-pc-windows-gnu. This pins it explicitly so
rem     a stray RUSTUP_TOOLCHAIN or a changed rustup default can
rem     never silently switch the build to MSVC (cl.exe is not
rem     installed on this machine, so an MSVC build fails).
rem   - Dev mode: tauri dev runs the Vite dev server and does not
rem     clean the dist directory, which avoids the sandbox problem
rem     that blocks `vite build`.
rem =============================================================

net session >nul 2>&1
if %errorlevel%==0 goto RUN

echo [dev-gnu] Requesting administrator privileges...
powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
exit /b 0

:RUN
set RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu
cd /d "%~dp0"

echo [dev-gnu] Elevated. Toolchain: %RUSTUP_TOOLCHAIN%
rustc -vV | findstr /C:"host:"
echo.
echo [dev-gnu] First build takes several minutes; later runs are incremental.
echo [dev-gnu] When the app window appears you can test the exclusion rules:
echo [dev-gnu]   copy a card-like number or an API token and watch the
echo [dev-gnu]   status bar for the exclusion hint and its keep button.
echo.

npm run tauri dev

echo.
echo [dev-gnu] Exited with code %errorlevel%
pause
