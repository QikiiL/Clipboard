@echo off
:: 以管理员身份启动开发环境(Win+V 集成等需要写 HKLM 注册表的功能要求整个应用提权)
net session >nul 2>&1
if %errorlevel%==0 (
    echo [dev-admin] Already elevated, starting dev...
    call "%~dp0dev.bat"
) else (
    echo [dev-admin] Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -FilePath '%~dp0dev.bat' -Verb RunAs"
)
