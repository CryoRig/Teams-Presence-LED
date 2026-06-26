param(
    [switch]$NoOptimizations = $false
)

$outputDir = ".\Publish"

Write-Host "Building Teams Presence Bridge (Rust)..." -ForegroundColor Cyan

# Ensure the publish directory exists
if (-not (Test-Path -Path $outputDir)) {
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
}

$baseArgs = @("build", "--release")

if ($NoOptimizations) {
    Write-Host "Mode: Unoptimized Release Build"
} else {
    Write-Host "Mode: Fully Optimized Size Build (LTO enabled, Symbols Stripped)"
}

Write-Host "Running: cargo $baseArgs" -ForegroundColor DarkGray
& cargo $baseArgs

if ($?) {
    $exePath = ".\target\release\teams-presence-bridge-rs.exe"
    if (Test-Path $exePath) {
        Copy-Item -Path $exePath -Destination "$outputDir\TeamsPresenceBridge.exe" -Force
        Write-Host "Build complete! Your executable is located at:" -ForegroundColor Green
        Write-Host (Resolve-Path "$outputDir\TeamsPresenceBridge.exe").Path -ForegroundColor Yellow
    } else {
        Write-Host "Build succeeded but executable not found at expected path." -ForegroundColor Red
    }
} else {
    Write-Host "Build failed." -ForegroundColor Red
}
