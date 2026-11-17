<#
.SYNOPSIS
  How much CPU and memory freedock uses: the dock process plus its WebView2 children, sampled over
  a period. Run it while the dock sits idle (shown or tucked) to check the performance budget.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\perf.ps1 -Seconds 60
#>
param(
    [int]$Seconds = 60,
    [string]$Name = "freedock.exe"
)

function Get-Tree {
    $all = Get-CimInstance Win32_Process
    # The dock itself (not the --guard helper), then every descendant (WebView2 processes).
    $roots = $all | Where-Object { $_.Name -eq $Name -and $_.CommandLine -notmatch '--guard|--snap|--restore-taskbar' }
    $ids = New-Object System.Collections.Generic.HashSet[int]
    foreach ($r in $roots) { [void]$ids.Add([int]$r.ProcessId) }
    do {
        $added = $false
        foreach ($p in $all) {
            if ($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)) { $added = $true }
        }
    } while ($added)
    return $ids
}

$ids = Get-Tree
if ($ids.Count -eq 0) { Write-Error "$Name is not running"; exit 1 }

function Get-Cpu($ids) {
    $t = 0.0
    foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p) { $t += $p.TotalProcessorTime.TotalMilliseconds }
    }
    return $t
}

$before = Get-Cpu $ids
$clock = [Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $Seconds
$after = Get-Cpu $ids
$elapsed = $clock.Elapsed.TotalMilliseconds

$mem = 0
foreach ($id in $ids) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $mem += $p.WorkingSet64 }
}

$pct = [math]::Round(100 * ($after - $before) / $elapsed, 2)
"{0} processes over {1} s: {2}% of one CPU core, {3} MB working set" -f $ids.Count, $Seconds, $pct, [math]::Round($mem / 1MB)
