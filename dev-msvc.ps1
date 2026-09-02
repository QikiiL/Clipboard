# 在 PowerShell 会话中加载 MSVC 编译环境(等效于 cmd 下 call vcvars64.bat)
# 用法: . .\dev-msvc.ps1   (必须点号 sourced,环境变量才能留在当前会话)
# 之后可直接 cargo check / cargo build / npm run tauri build
#
# 背景:Git Bash 自带的 /usr/bin/link(coreutils)会遮蔽 MSVC 的 link.exe,
# 在 bash 里直接跑 cargo 会报 "link: extra operand"。本机 cmd.exe 被安全策略
# 拦截、Enter-VsDevShell 模块加载失败,故手动拼 vcvars64.bat 的核心环境变量。
# 工具集版本升级后需同步改下面两个变量。

$MSVC = "D:\Microsoft\VisualStudio\VC\Tools\MSVC\14.51.36231"
$SDK_VER = "10.0.26100.0"
$SDK = "C:\Program Files (x86)\Windows Kits\10"

$env:PATH = "$MSVC\bin\Hostx64\x64;$env:PATH"
$env:LIB = "$MSVC\lib\x64;$SDK\Lib\$SDK_VER\um\x64;$SDK\Lib\$SDK_VER\ucrt\x64"
$env:INCLUDE = "$MSVC\include;$SDK\Include\$SDK_VER\um;$SDK\Include\$SDK_VER\ucrt;$SDK\Include\$SDK_VER\shared;$SDK\Include\$SDK_VER\winrt"

Write-Host "MSVC env loaded: link.exe -> $((Get-Command link.exe).Source)"
