# Live bezel repro loop - drives the REAL dock window on this machine.
#
# Phase A (bug 1): mode-switch stress fixed->bezel->auto-hide against a live
#   docked window; RED on panic/abort/timeout (frozen white bar / hung save).
# Phase B (bugs 2+3): holds a live bezel tab and clicks it through CDP;
#   RED when the tab will not open (not clickable) or a click destroys it.
#
# Isolation: the test instance runs with LOCALAPPDATA pointed at a throwaway
# dir, autostart forced off - your data and login entries are untouched.
# Disruption: Phase A/B dock real AppBars and move windows on your screen
# for ~2 minutes. The script stops any running sprout-windows-desktop first
# (single-instance would hijack the run) - relaunch your dev app afterwards.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File tools\repro-bezel-live.ps1 [-SkipBuild] [-CdpPort 9333]
param(
    [switch]$SkipBuild,
    [int]$CdpPort = 9333,
    [int]$StressIters = 30,
    [int]$StressIntervalMs = 150
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $repo "src-tauri\target\debug\sprout-windows-desktop.exe"

if (-not $SkipBuild) {
    Write-Host "== vite build (bake frontend into the exe) =="
    Push-Location $repo
    cmd /c "npm.cmd run build 2>&1" | Select-Object -Last 4
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "VITE BUILD FAILED"; exit 2 }
    Pop-Location
    Write-Host "== cargo build (debug) =="
    Push-Location (Join-Path $repo "src-tauri")
    cmd /c "cargo build 2>&1" | Select-Object -Last 3
    $buildOk = ($LASTEXITCODE -eq 0)
    Pop-Location
    if (-not $buildOk) { Write-Host "CARGO BUILD FAILED"; exit 2 }
}
if (-not (Test-Path $exe)) { Write-Host "exe not found: $exe"; exit 2 }

$existing = Get-Process -Name "sprout-windows-desktop" -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "stopping running instance (pid $($existing.Id)) for the live run - relaunch your dev app afterwards"
    Stop-Process -Id $existing.Id -Force
    Start-Sleep -Seconds 2
}

# The debug exe serves its frontend from the vite dev server (devUrl): the
# app windows stay blank without it, so make sure it is up. Left running
# afterwards - the dev workflow needs it anyway.
# Vite listens on IPv6 loopback here: probe HTTP via "localhost" (which
# resolves there), never a raw IPv4 TCP connect.
function Test-ViteUp() {
    try {
        $r = Invoke-WebRequest -Uri "http://localhost:1420/" -TimeoutSec 5 -UseBasicParsing
        return $r.StatusCode -lt 500
    } catch { return $false }
}
if (-not (Test-ViteUp)) {
    Write-Host "starting vite dev server (serves the debug exe frontend)"
    Push-Location $repo
    Start-Process -FilePath "cmd.exe" -ArgumentList "/c npm.cmd run dev" -WindowStyle Hidden | Out-Null
    Pop-Location
    $deadline = (Get-Date).AddSeconds(120)
    while (-not (Test-ViteUp) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 1000 }
    if (-not (Test-ViteUp)) { Write-Host "VITE NEVER CAME UP"; exit 2 }
    Write-Host "vite is up"
}

$iso = Join-Path ([IO.Path]::GetTempPath()) ("sprout-bezel-live-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $iso | Out-Null
Write-Host "isolated profile: $iso"

$red = 0
try {
    # ---------- Phase A: bezel mode-switch stress ----------
    $marker = Join-Path $iso "stress.json"
    $logErr = Join-Path $iso "stress.err.log"
    $env:LOCALAPPDATA = $iso
    $env:SPROUT_DOCK_STRESS = "1"
    $env:SPROUT_DOCK_STRESS_ITERS = "$StressIters"
    $env:SPROUT_DOCK_STRESS_MS = "$StressIntervalMs"
    $env:SPROUT_DOCK_STRESS_MODES = "docked,fixed,bezel,auto-hide,floating,docked,bezel,floating"
    $env:SPROUT_DOCK_STRESS_RESULT = $marker
    $env:RUST_BACKTRACE = "1"
    Write-Host "== Phase A: stress dock/mode cycle x$StressIters =="
    $p = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden -RedirectStandardError $logErr
    if (-not $p.WaitForExit(180000)) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Write-Host "Phase A: RED (timeout - dock froze mid-switch)"
        $red++
    } else {
        Start-Sleep -Milliseconds 300
        $body = if (Test-Path $marker) { (Get-Content $marker -Raw).Trim() } else { "" }
        $errText = if (Test-Path $logErr) { Get-Content $logErr -Raw } else { "" }
        $panicked = $errText -match "panicked|abort|STATUS_STACK_BUFFER_OVERRUN|STATUS_ACCESS_VIOLATION"
        if ($body.StartsWith("PASS") -and -not $panicked) {
            Write-Host "Phase A: green ($body)"
        } else {
            Write-Host "Phase A: RED (exit=$($p.ExitCode) marker='$body' panicked=$panicked)"
            $red++
        }
    }

    # ---------- Phase B: live tab clicks ----------
    Write-Host "== Phase B: hold bezel tab + CDP clicks =="
    $env:SPROUT_DOCK_STRESS = ""
    $env:SPROUT_BEZEL_HOLD = "1"
    $env:SPROUT_BEZEL_HOLD_SECS = "150"
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort"
    $holdErr = Join-Path $iso "hold.err.log"
    $hold = Start-Process -FilePath $exe -PassThru -WindowStyle Hidden -RedirectStandardError $holdErr
    try {
        Push-Location $repo
        # Foreground the tab window first: CDP synthetic input is dropped
        # while the WebView is unfocused, which reads as lost clicks.
        $wshell = New-Object -ComObject WScript.Shell
        $deadline = (Get-Date).AddSeconds(90)
        $qlTarget = $null
        while (-not $qlTarget -and (Get-Date) -lt $deadline) {
            try {
                $targets = Invoke-RestMethod "http://127.0.0.1:$CdpPort/json" -TimeoutSec 3
                $qlTarget = $targets | Where-Object { $_.url -like "*quick-launch*" } | Select-Object -First 1
            } catch {}
            if (-not $qlTarget) { Start-Sleep -Milliseconds 1000 }
        }
        if (-not $qlTarget) { Write-Host "quick-launch CDP target never appeared"; $red++ }
        else {
        [void]$wshell.AppActivate("Sprout $([char]0x2014) Quick Launch")
        Start-Sleep -Seconds 1
        cmd /c "node tools\repro-bezel-click.mjs --port $CdpPort 2>&1"
        if ($LASTEXITCODE -ne 0) { $red++ }
        }
        Pop-Location
    } finally {
        Stop-Process -Id $hold.Id -Force -ErrorAction SilentlyContinue
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host "--- hold backend log tail (evidence preserved in $iso) ---"
        Get-Content $holdErr -ErrorAction SilentlyContinue | Select-Object -Last 12 | ForEach-Object { Write-Host "    $_" }
    }
} finally {
    if ($red -gt 0) {
        $keep = Join-Path ([IO.Path]::GetTempPath()) ("sprout-bezel-live-keep-" + [DateTime]::Now.ToString("HHmmss"))
        Copy-Item $iso $keep -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "RED evidence preserved at $keep"
        Remove-Item $iso -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        Remove-Item $iso -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if ($red -gt 0) { Write-Host "== LIVE VERDICT: RED =="; exit 1 }
Write-Host "== LIVE VERDICT: GREEN =="
exit 0
