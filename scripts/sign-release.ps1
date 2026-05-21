param(
  [Parameter(Mandatory = $true)]
  [string]$CertThumbprint,

  [string]$BundleRoot = (Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle"),

  [string]$TimestampUrl = "http://timestamp.digicert.com",

  [switch]$IncludeMsi = $true,
  [switch]$IncludeExe = $true
)

$ErrorActionPreference = "Stop"

function Write-Step($Message) {
  Write-Host "[sign-release] $Message"
}

function Resolve-SignTool {
  $tool = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($tool) { return $tool.Source }

  $kits = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
    "${env:ProgramFiles}\Windows Kits\10\bin"
  ) | Where-Object { $_ -and (Test-Path $_) }

  foreach ($root in $kits) {
    $candidate = Get-ChildItem -Path $root -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
      Sort-Object FullName -Descending |
      Select-Object -First 1
    if ($candidate) { return $candidate.FullName }
  }

  throw "signtool.exe not found. Install Windows SDK Signing Tools."
}

function Get-Targets {
  param(
    [string]$Root,
    [bool]$IncludeMsiFiles,
    [bool]$IncludeExeFiles
  )

  if (-not (Test-Path $Root)) {
    throw "Bundle root not found: $Root"
  }

  $patterns = @()
  if ($IncludeMsiFiles) { $patterns += "*.msi" }
  if ($IncludeExeFiles) { $patterns += "*.exe" }
  if ($patterns.Count -eq 0) {
    throw "No file types selected. Enable IncludeMsi and/or IncludeExe."
  }

  $files = @()
  foreach ($pattern in $patterns) {
    $files += Get-ChildItem -Path $Root -Recurse -File -Filter $pattern -ErrorAction SilentlyContinue
  }

  return $files | Sort-Object FullName -Unique
}

$signTool = Resolve-SignTool
$resolvedBundleRoot = Resolve-Path $BundleRoot
$targets = Get-Targets -Root $resolvedBundleRoot -IncludeMsiFiles:$IncludeMsi -IncludeExeFiles:$IncludeExe

if ($targets.Count -eq 0) {
  throw "No installer artifacts found under $resolvedBundleRoot"
}

Write-Step "Using signtool: $signTool"
Write-Step "Signing $($targets.Count) artifact(s) from $resolvedBundleRoot"

foreach ($file in $targets) {
  Write-Step "Signing $($file.FullName)"
  & $signTool sign `
    /sha1 $CertThumbprint `
    /fd SHA256 `
    /tr $TimestampUrl `
    /td SHA256 `
    /v `
    $file.FullName
}

Write-Step "Signature verification pass"
foreach ($file in $targets) {
  & $signTool verify /pa /v $file.FullName | Out-Null
}

Write-Step "Done. All selected artifacts are signed and verified."
