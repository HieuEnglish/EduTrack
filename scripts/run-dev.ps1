param(
  [switch]$Browser,
  [switch]$Doctor
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Frontend = Join-Path $Root "frontend"
$Tauri = Join-Path $Root "src-tauri"
$DevCache = Join-Path $Root ".dev-cache"
$CargoTargetDir = Join-Path $DevCache "cargo-target"
$ExpectedAppExe = Join-Path $CargoTargetDir "debug\tauri-app.exe"
$PreferredPort = 5173
$DesktopPort = 5176

function Write-Step($Message) {
  Write-Host "[EduTrack] $Message"
}

function Test-Command($Name) {
  return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Test-Port($PortNumber) {
  $connection = Get-NetTCPConnection -LocalPort $PortNumber -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
  return [bool]$connection
}

function Test-FileLocked($Path) {
  if (-not (Test-Path $Path)) {
    return $false
  }
  try {
    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::None)
    $stream.Close()
    return $false
  } catch [System.IO.IOException] {
    return $true
  }
}

function Get-PortOwner($PortNumber) {
  $line = netstat -ano | Select-String "127\.0\.0\.1:$PortNumber\s+.*LISTENING" | Select-Object -First 1
  if (-not $line) {
    return $null
  }
  $parts = ($line.ToString() -split "\s+") | Where-Object { $_ }
  if ($parts.Length -lt 5) {
    return $null
  }
  return [int]$parts[-1]
}

function Stop-PortOwner($PortNumber) {
  $ownerPid = Get-PortOwner $PortNumber
  if ($ownerPid) {
    Write-Step "Stopping stale process $ownerPid on port $PortNumber."
    Stop-Process -Id $ownerPid -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
  }
}

function Test-EduTrackUi($PortNumber) {
  try {
    $response = Invoke-WebRequest -Uri "http://127.0.0.1:$PortNumber" -UseBasicParsing -TimeoutSec 2
    return $response.Content -match "<title>EduTrack</title>" -or $response.Content -match "EduTrack full implementation"
  } catch {
    return $false
  }
}

function Get-FreePort($StartPort) {
  for ($candidate = $StartPort; $candidate -le 5299; $candidate++) {
    if (-not (Test-Port $candidate)) {
      return $candidate
    }
  }
  throw "No free local UI port found between $StartPort and 5299."
}

function Show-AvgChecklist() {
  Write-Host ""
  Write-Host "AVG Exception Checklist (copy these into AVG > Menu > Settings > Exceptions):"
  Write-Host "  $Root"
  Write-Host "  $DevCache"
  Write-Host "  $CargoTargetDir"
  Write-Host "  $Tauri"
  Write-Host "  $(Join-Path $Frontend 'node_modules')"
  Write-Host "  $ExpectedAppExe"
  Write-Host ""
}

if (-not (Test-Path $Frontend)) {
  throw "Frontend folder not found: $Frontend"
}

New-Item -ItemType Directory -Path $DevCache -Force | Out-Null
New-Item -ItemType Directory -Path $CargoTargetDir -Force | Out-Null

if ($Doctor) {
  Write-Step "Doctor mode: printing AVG-safe exclusions and local lock status."
  Show-AvgChecklist
  if (Test-FileLocked $ExpectedAppExe) {
    Write-Step "Detected lock on $ExpectedAppExe. This is usually antivirus real-time scanning."
    Write-Host "Quick recovery:"
    Write-Host "  1. End task: tauri-app.exe and cargo-tauri.exe"
    Write-Host "  2. In AVG, add the exceptions listed above"
    Write-Host "  3. Re-run: .\\run.bat"
    exit 1
  }
  Write-Step "No lock detected on expected app binary path."
  exit 0
}

if (-not (Test-Command "node")) {
  throw "Node.js is required. Install Node.js 18+ and run this again."
}

if (-not (Test-Command "npm")) {
  throw "npm is required. Install Node.js/npm and run this again."
}

if (-not $Browser) {
  if (-not (Test-Command "cargo")) {
    throw "Rust/Cargo is required to run the EduTrack desktop app."
  }

  Push-Location $Frontend
  try {
    if (-not (Test-Path "node_modules")) {
      Write-Step "Installing frontend dependencies..."
      npm install
    } else {
      Write-Step "Frontend dependencies already installed."
    }
  } finally {
    Pop-Location
  }

  $RunningApp = Get-Process tauri-app -ErrorAction SilentlyContinue
  if ($RunningApp) {
    Write-Step "Restarting existing EduTrack desktop app so the latest backend commands are loaded."
    $RunningApp | Stop-Process -Force
    Start-Sleep -Seconds 1
  }

  $RunningTauri = Get-Process cargo-tauri -ErrorAction SilentlyContinue
  if ($RunningTauri) {
    $RunningTauri | Stop-Process -Force
    Start-Sleep -Milliseconds 500
  }

  if ((Test-Port $DesktopPort) -and -not (Test-EduTrackUi $DesktopPort)) {
    Write-Step "Port $DesktopPort is occupied but not responding as EduTrack; attempting a clean desktop restart."
    Stop-PortOwner $DesktopPort
  }

  if (Test-FileLocked $ExpectedAppExe) {
    Write-Step "Detected antivirus lock on $ExpectedAppExe."
    Show-AvgChecklist
    throw "App binary is locked before startup. Configure AVG exceptions, then rerun .\\run.bat."
  }

  Write-Step "Using stable Rust target directory: $CargoTargetDir"
  $TauriCommand = "cd '$Tauri'; `$env:CARGO_TARGET_DIR='$CargoTargetDir'; `$env:CARGO_HTTP_CHECK_REVOKE='false'; cargo tauri dev --no-watch"
  if (Test-EduTrackUi $DesktopPort) {
    $ExistingConfig = Join-Path $Tauri "tauri.dev-existing.conf.json"
    $TauriCommand = "cd '$Tauri'; `$env:CARGO_TARGET_DIR='$CargoTargetDir'; `$env:CARGO_HTTP_CHECK_REVOKE='false'; cargo tauri dev --no-watch --config '$ExistingConfig'"
    Write-Step "Reusing existing EduTrack dev server at http://127.0.0.1:$DesktopPort."
  }

  Write-Step "Starting EduTrack desktop app. Provider probing and model listing are enabled in this mode."
  Start-Process `
    -FilePath "powershell.exe" `
    -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $TauriCommand `
    -WindowStyle Minimized
  Write-Step "Desktop app launch requested. If the window does not appear, check the minimized PowerShell window for build output."
  exit 0
}

Push-Location $Frontend
try {
  if (-not (Test-Path "node_modules")) {
    Write-Step "Installing frontend dependencies..."
    npm install
  } else {
    Write-Step "Frontend dependencies already installed."
  }

  if ((Test-Port $PreferredPort) -and (Test-EduTrackUi $PreferredPort)) {
    $Port = $PreferredPort
    $Url = "http://127.0.0.1:$Port"
    Write-Step "EduTrack UI server already running at $Url."
  } else {
    if (Test-Port $PreferredPort) {
      Write-Step "Port $PreferredPort is in use by another app; choosing a free EduTrack port."
      $Port = Get-FreePort ($PreferredPort + 1)
    } else {
      $Port = $PreferredPort
    }
    $Url = "http://127.0.0.1:$Port"
    Write-Step "Starting UI server at $Url..."
    Start-Process `
      -FilePath "powershell.exe" `
      -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", "cd '$Frontend'; npm run dev -- --host 127.0.0.1 --port $Port --strictPort" `
      -WindowStyle Minimized

    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline) {
      if ((Test-Port $Port) -and (Test-EduTrackUi $Port)) {
        break
      }
      Start-Sleep -Milliseconds 500
    }

    if (-not ((Test-Port $Port) -and (Test-EduTrackUi $Port))) {
      throw "UI server did not start on port $Port within 20 seconds."
    }
  }

  Write-Step "Opening UI..."
  Start-Process $Url
  Write-Step "Ready: $Url"
} finally {
  Pop-Location
}
