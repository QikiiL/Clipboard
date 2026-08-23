# Load VS Build Tools environment into PowerShell
$vcvars = "D:\Microsoft\VisualStudio\VC\Auxiliary\Build\vcvars64.bat"

# Run vcvars64.bat and capture environment variables
cmd /c "`"$vcvars`" > nul 2>&1 && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
    }
}

# Verify link.exe is available
$linkPath = (Get-Command link.exe -ErrorAction SilentlyContinue).Source
if ($linkPath -and $linkPath -like "*MSVC*") {
    Write-Host "MSVC link.exe found: $linkPath" -ForegroundColor Green
} else {
    Write-Host "WARNING: MSVC link.exe not in PATH" -ForegroundColor Yellow
}

# Run tauri dev
Set-Location "D:\copy\clipboard-manager-tauri"
npm run tauri dev
