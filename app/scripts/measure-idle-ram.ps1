# Launch the release exe, wait N seconds, then sum memory of the app process + all descendants
# (WebView2 msedgewebview2.exe children). Usage: powershell -File measure-idle-ram.ps1 [-Seconds 60]
param(
  [int]$Seconds = 60,
  [string]$Exe = (Join-Path $PSScriptRoot '..\src-tauri\target\release\cruxvid.exe'),
  [int]$Runs = 1
)
$ErrorActionPreference = 'Stop'
foreach ($run in 1..$Runs) {
  $userData = Join-Path $env:TEMP ("cruxvid-ram-" + [guid]::NewGuid())
  $env:WEBVIEW2_USER_DATA_FOLDER = $userData   # fresh profile = cold-start number
  $p = Start-Process -FilePath $Exe -PassThru
  Start-Sleep -Seconds $Seconds
  $all = Get-CimInstance Win32_Process
  $ids = [System.Collections.Generic.HashSet[uint32]]::new()
  $null = $ids.Add([uint32]$p.Id)
  do { $n = $ids.Count
    foreach ($q in $all) { if ($ids.Contains([uint32]$q.ParentProcessId)) { $null = $ids.Add([uint32]$q.ProcessId) } }
  } while ($ids.Count -ne $n)
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids.Contains([uint32]$_.IDProcess) }
  $ws   = ($perf | Measure-Object WorkingSet -Sum).Sum
  $priv = ($perf | Measure-Object WorkingSetPrivate -Sum).Sum
  "run {0}: processes={1} workingSet={2:N1} MiB privateWorkingSet={3:N1} MiB" -f $run, $perf.Count, ($ws/1MB), ($priv/1MB)
  $perf | ForEach-Object { "  {0,-24} pid={1,6} ws={2,7:N1} MiB private={3,7:N1} MiB" -f $_.Name, $_.IDProcess, ($_.WorkingSet/1MB), ($_.WorkingSetPrivate/1MB) }
  taskkill /PID $p.Id /T /F | Out-Null
  Start-Sleep -Seconds 2
  Remove-Item -Recurse -Force $userData -ErrorAction SilentlyContinue
}
