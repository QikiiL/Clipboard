# Measures the clipboard-manager process tree memory (working set + private)
$apps = Get-Process clipboard-manager-tauri -ErrorAction SilentlyContinue
if (-not $apps) { Write-Output 'app not running'; exit 1 }
$total = 0; $priv = 0
$apps | ForEach-Object {
  '{0,-28} pid={1,-7} WS={2,7:N1}MB Priv={3,7:N1}MB' -f $_.Name, $_.Id, ($_.WorkingSet64/1MB), ($_.PrivateMemorySize64/1MB)
  $total += $_.WorkingSet64; $priv += $_.PrivateMemorySize64
}
$appIds = @($apps.Id)
$kids = Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object {
  $p = $_; $found = $false
  while ($p.ParentProcessId) {
    if ($p.ParentProcessId -in $appIds) { $found = $true; break }
    $p = Get-CimInstance Win32_Process -Filter ('ProcessId=' + $p.ParentProcessId) -ErrorAction SilentlyContinue
    if (-not $p -or $p.Name -eq 'explorer.exe') { break }
  }
  $found
}
$kids | ForEach-Object {
  $proc = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue
  if ($proc) {
    '{0,-28} pid={1,-7} WS={2,7:N1}MB Priv={3,7:N1}MB' -f $proc.Name, $proc.Id, ($proc.WorkingSet64/1MB), ($proc.PrivateMemorySize64/1MB)
    $total += $proc.WorkingSet64; $priv += $proc.PrivateMemorySize64
  }
}
''
'TOTAL: WorkingSet {0:N1} MB / Private {1:N1} MB' -f ($total/1MB), ($priv/1MB)
