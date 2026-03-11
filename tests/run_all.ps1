# Quin Master Test Runner (PowerShell)
$QuinExecutable = "target/debug/quin.exe"

if (-not (Test-Path $QuinExecutable)) {
    Write-Host "Quin executable not found! Building..." -ForegroundColor Yellow
    cargo build
}

$TestFiles = Get-ChildItem -Path tests -Filter *.qn -Recurse
$Passed = 0
$Failed = 0

Write-Host "--- Starting Quin Test Suite ---" -ForegroundColor Cyan

foreach ($file in $TestFiles) {
    Write-Host "Running: $($file.Name)... " -NoNewline
    
    $output = & $QuinExecutable $file.FullName 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "PASS" -ForegroundColor Green
        $Passed++
    } else {
        Write-Host "FAIL" -ForegroundColor Red
        Write-Host "Error details:" -ForegroundColor Gray
        Write-Host $output
        $Failed++
    }
}

Write-Host "`n--- Test Summary ---" -ForegroundColor Cyan
Write-Host "Passed: $Passed" -ForegroundColor Green
$FailedColor = "Gray"
if ($Failed -gt 0) { $FailedColor = "Red" }
Write-Host "Failed: $Failed" -ForegroundColor $FailedColor

if ($Failed -gt 0) {
    exit 1
} else {
    exit 0
}
