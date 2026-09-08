#Requires -Version 5.1
<#
.SYNOPSIS
    Build sparse MSIX package identity (clipboard-identity.msix + clipboard-identity.cer).
#>

$ErrorActionPreference = "Stop"

$ScriptDir   = $PSScriptRoot
$Manifest    = Join-Path $ScriptDir "AppxManifest.xml"
$IconsSrc    = Join-Path $ScriptDir "..\icons"
$AssetsDir   = Join-Path $ScriptDir "Assets"
$MsixName    = "clipboard-identity.msix"
$CerName     = "clipboard-identity.cer"
$PfxName     = "clipboard-identity.pfx"
$MsixPath    = Join-Path $ScriptDir $MsixName
$CerPath     = Join-Path $ScriptDir $CerName
$PfxPath     = Join-Path $ScriptDir $PfxName
$PfxPassword = "identity"

$SdkBase = "C:\Program Files (x86)\Windows Kits\10\bin"
$SdkVer  = "10.0.26100.0"
$MakeAppx = Join-Path $SdkBase "$SdkVer\x64\makeappx.exe"
$SignTool = Join-Path $SdkBase "$SdkVer\x64\signtool.exe"

if (-not (Test-Path $MakeAppx)) { throw "makeappx.exe not found: $MakeAppx" }
if (-not (Test-Path $SignTool)) { throw "signtool.exe not found: $SignTool" }

# 1. Certificate: create if absent, reuse if present
$CertSubject = "CN=ClipboardManager"
$CertFriendly = "ClipboardManager Identity"
# KeyUsage 必须在脚本作用域声明(本机实测:块内声明的数组变量传给 -KeyUsage
# 会被忽略回退默认 DigitalSignature+KeyEncipherment)。CertSign 是本机
# PowerShell 5.1 的合法枚举值(KeyCertSign 非法)。
$keyUsages = @("DigitalSignature", "CertSign", "CRLSign")

$existing = Get-ChildItem Cert:\CurrentUser\My | Where-Object {
    $_.Subject -eq $CertSubject -and $_.FriendlyName -eq $CertFriendly
}

$securePwd = New-Object System.Security.SecureString
$PfxPassword.ToCharArray() | ForEach-Object { $securePwd.AppendChar($_) }

if ($existing) {
    Write-Host "[build-identity] reuse existing cert: $CertSubject"
    $cert = $existing[0]
} elseif (Test-Path $PfxPath) {
    # 证书已从存储丢失(重装/清理)但 pfx 文件还在:直接从 pfx 导回并复用,
    # 避免新建证书与旧 pfx/cer/msix 签名不一致
    Write-Host "[build-identity] cert not in store, re-import from existing pfx"
    $cert = Import-PfxCertificate -FilePath $PfxPath -CertStoreLocation Cert:\CurrentUser\My -Password $securePwd
    if (-not $cert) { throw "Import-PfxCertificate returned nothing" }
} else {
    Write-Host "[build-identity] create self-signed cert: $CertSubject"
    # 注意:稀疏包签名校验要求根证书具备 BasicConstraints CA:TRUE,
    # 且 KeyUsage 含 CertSign,否则 AddPackageByUriAsync 报 0x800B0109
    # (CERT_E_UNTRUSTEDROOT)。故证书必须带 CA:TRUE 与 CertSign/KeyCertSign。
    # 注意:PowerShell 5.1 的 New-SelfSignedCertificate -KeyUsage 仅接受
    # Microsoft.CertificateServices.Commands.KeyUsage 枚举;本机实测合法值为
    # "CertSign"(而非 "KeyCertSign"),写错会导致整组 KeyUsage 被忽略回退默认。
    # BasicConstraints 必须用原始 DER 十六进制 {hex}30030101ff (=CA:TRUE),
    # 单纯 {text}CA:TRUE 在本机会被忽略、回退 End Entity,导致稀疏包校验
    # 报 0x800B0109(CERT_E_UNTRUSTEDROOT)。
    # KeyUsage 用简单字符串数组:本机实测(独立 .ps1 复现)DigitalSignature +
    # CertSign + CRLSign 在 -File 上下文可被正确识别;BasicConstraints 必须用
    # 原始 DER 十六进制 {hex}30030101ff(=CA:TRUE),{text}CA:TRUE 会被忽略。
    # 注意:这里不能用反引号(`)换行,反引号后的隐藏字符会破坏参数绑定,
    # 导致 -KeyUsage 被静默忽略、回退默认(DigitalSignature+KeyEncipherment)。
    $cert = New-SelfSignedCertificate -Type Custom -Subject $CertSubject -KeyUsage $keyUsages -FriendlyName $CertFriendly -CertStoreLocation Cert:\CurrentUser\My -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={hex}30030101ff")
    if (-not $cert) { throw "New-SelfSignedCertificate returned nothing" }
}

# 2. Export pfx + cer (skip if already present)
if (-not (Test-Path $PfxPath) -or -not (Test-Path $CerPath)) {
    Write-Host "[build-identity] export pfx/cer to script dir"
    $securePwd = New-Object System.Security.SecureString
    $PfxPassword.ToCharArray() | ForEach-Object { $securePwd.AppendChar($_) }
    Export-PfxCertificate -Cert $cert -FilePath $PfxPath -Password $securePwd | Out-Null
    Export-Certificate -Cert $cert -FilePath $CerPath | Out-Null
} else {
    Write-Host "[build-identity] pfx/cer already exist, skip export"
}

# 3. Prepare Assets (copy pngs from icons; size not strict)
if (-not (Test-Path $AssetsDir)) { New-Item -ItemType Directory -Path $AssetsDir | Out-Null }
$copyMap = @{
    "StoreLogo.png"         = "storelogo.png"
    "Square150x150Logo.png" = "logo150.png"
    "Square44x44Logo.png"   = "logo44.png"
}
foreach ($srcName in $copyMap.Keys) {
    $src = Join-Path $IconsSrc $srcName
    $dst = Join-Path $AssetsDir $copyMap[$srcName]
    if (-not (Test-Path $src)) { throw "icon source not found: $src" }
    Copy-Item -Force $src $dst
}

# 4. makeappx pack (mapping file, absolute paths)
$mapping = Join-Path $ScriptDir "files.txt"
$lines = @("[Files]")
$lines += "`"$Manifest`" `"AppxManifest.xml`""
foreach ($png in @("storelogo.png", "logo150.png", "logo44.png")) {
    $abs = Join-Path $AssetsDir $png
    $lines += "`"$abs`" `"Assets\$png`""
}
Set-Content -Path $mapping -Value $lines -Encoding UTF8

if (Test-Path $MsixPath) { Remove-Item $MsixPath -Force }

Write-Host "[build-identity] makeappx pack -> $MsixName"
& $MakeAppx pack /nv /o /f "$mapping" /p "$MsixPath"
if ($LASTEXITCODE -ne 0) { throw "makeappx failed (exit $LASTEXITCODE)" }

# 5. signtool sign
Write-Host "[build-identity] signtool sign"
& $SignTool sign /fd SHA256 /a /f "$PfxPath" /p $PfxPassword "$MsixPath"
if ($LASTEXITCODE -ne 0) { throw "signtool failed (exit $LASTEXITCODE)" }

Write-Host ""
Write-Host "[build-identity] done."
Write-Host "  msix: $MsixPath"
Write-Host "  cer : $CerPath"
