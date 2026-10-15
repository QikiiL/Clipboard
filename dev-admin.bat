@echo off
:: 以管理员身份启动开发环境(Win+V 集成等需要写 HKLM 注册表的功能要求整个应用提权)
net session >nul 2>&1
if %errorlevel%==0 (
    echo [dev-admin] Already elevated, starting dev...
    call "%~dp0dev.bat"
) else (
    echo [dev-admin] Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -FilePath '%~dp0dev.bat' -Verb RunAs"
    echo.
    echo [dev-admin] 已请求管理员权限：请在弹出的 UAC 窗口中点「是」，dev 会在新窗口里运行。
    echo [dev-admin] 若没有出现新窗口，说明提权被取消；本窗口保留，按任意键关闭。
    pause
)
