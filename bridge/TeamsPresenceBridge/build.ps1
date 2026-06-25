param(
    [switch]$SelfContained = $true,
    [switch]$NoPdb = $true
)

$projectPath = "TeamsPresenceBridge.csproj"
$outputDir = ".\Publish"

Write-Host "Building Teams Presence Bridge..." -ForegroundColor Cyan

# Start building the list of arguments
$baseArgs = @(
    "publish", $projectPath, 
    "-c", "Release", 
    "-r", "win-x64", 
    "-p:PublishSingleFile=true", 
    "-o", $outputDir
)

if ($SelfContained) {
    Write-Host "Mode: Self-Contained Single File (No dependencies required on target PC)"
    $baseArgs += @("--self-contained", "true", "-p:IncludeNativeLibrariesForSelfExtract=true")
} else {
    Write-Host "Mode: Framework-Dependent Single File (Target PC requires .NET 10 Desktop Runtime)"
    $baseArgs += @("--self-contained", "false")
}

if ($NoPdb) {
    Write-Host "PDB: Excluded (No debugging symbols will be generated)"
    $baseArgs += "-p:DebugType=None"
} else {
    Write-Host "PDB: Included (Debugging symbols will be generated)"
}

Write-Host "Running: dotnet $baseArgs" -ForegroundColor DarkGray
& dotnet $baseArgs

if ($?) {
    Write-Host "Build complete! Your executable is located at:" -ForegroundColor Green
    Write-Host (Resolve-Path "$outputDir\TeamsPresenceBridge.exe").Path -ForegroundColor Yellow
} else {
    Write-Host "Build failed." -ForegroundColor Red
}
