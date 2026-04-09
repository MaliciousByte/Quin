# Quin Master Test Runner (PowerShell)
$QuinExecutable = "target/release/quin.exe"

if (-not (Test-Path $QuinExecutable)) {
    Write-Host "Quin executable not found! Building release..." -ForegroundColor Yellow
    cargo build --release
}

$TestFiles = Get-ChildItem -Path tests -Filter *.qn -Recurse | Where-Object {
    $_.FullName -notmatch "benchmarks" -and
    $_.Name -notin "a.qn", "b.qn", "helpers.qn"
}



$Passed = 0
$Failed = 0

Write-Host ""
Write-Host "--- Quin Test Suite ---" -ForegroundColor Cyan

foreach ($file in $TestFiles) {
    Write-Host "Running: $($file.Name)... " -NoNewline
    $output = & $QuinExecutable $file.FullName 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "PASS" -ForegroundColor Green
        $Passed++
    } else {
        Write-Host "FAIL" -ForegroundColor Red
        Write-Host $output -ForegroundColor Gray
        $Failed++
    }
}

Write-Host ""
Write-Host "--- Test Summary ---" -ForegroundColor Cyan
Write-Host "Passed: $Passed" -ForegroundColor Green
if ($Failed -gt 0) {
    Write-Host "Failed: $Failed" -ForegroundColor Red
} else {
    Write-Host "Failed: $Failed" -ForegroundColor Gray
}

# ── Quin Benchmarks ──────────────────────────────────────────────────────────
$BenchDir = "tests/benchmarks"
$BenchDirs = @()
if (Test-Path $BenchDir) {
    $BenchDirs = Get-ChildItem -Path $BenchDir -Directory
}

if ($BenchDirs.Count -gt 0) {
    Write-Host ""
    Write-Host "--- Quin Benchmarks (JIT steady-state) ---" -ForegroundColor Magenta

    foreach ($dir in $BenchDirs) {
        $benchFiles = Get-ChildItem -Path $dir.FullName -Filter *.qn
        foreach ($bench in $benchFiles) {
            Write-Host "Bench: $($dir.Name)/$($bench.Name)... " -NoNewline
            $warmup = & $QuinExecutable $bench.FullName 2>&1
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $result = & $QuinExecutable $bench.FullName 2>&1
            $sw.Stop()
            if ($LASTEXITCODE -eq 0) {
                $ms = $sw.Elapsed.TotalMilliseconds
                Write-Host ("{0:F1}ms" -f $ms) -ForegroundColor Yellow
                if ($result) { Write-Host $result -ForegroundColor DarkGray }
            } else {
                Write-Host "ERROR" -ForegroundColor Red
                Write-Host $result -ForegroundColor Gray
            }
        }
    }
}

# ── Cross-language Comparison ────────────────────────────────────────────────
if ($BenchDirs.Count -gt 0) {
    Write-Host ""
    Write-Host "--- Cross-Language Comparison ---" -ForegroundColor Cyan
    Write-Host "  (Quin = internal JIT clock; Python/JS = internal loop timer)" -ForegroundColor DarkGray

    foreach ($dir in $BenchDirs) {
        $benchName = $dir.Name
        $quinBench = Get-ChildItem -Path $dir.FullName -Filter *.qn | Select-Object -First 1
        $pyBench   = Get-ChildItem -Path $dir.FullName -Filter *.py  | Select-Object -First 1
        $jsBench   = Get-ChildItem -Path $dir.FullName -Filter *.js  | Select-Object -First 1

        if (-not $quinBench) { continue }

        Write-Host ""
        Write-Host "  -- $benchName --" -ForegroundColor Cyan

        # Quin — warm up then read internal JIT time
        $null = & $QuinExecutable $quinBench    .FullName 2>&1
        $quinOut = (& $QuinExecutable $quinBench.FullName 2>&1) -join "`n"
        $quinMs = $null
        if ($quinOut -match "JIT time:\s*([\d.]+)s") {
            $quinMs = [double]$matches[1] * 1000
            Write-Host ("    Quin   (JIT):  {0,8:F3}ms" -f $quinMs) -ForegroundColor Magenta
        }

        # Python
        if ($pyBench) {
            $pyOut = (& python $pyBench.FullName 2>&1) -join "`n"
            if ($pyOut -match "Python Time:\s*([\d.]+)s") {
                $pyMs = [double]$matches[1] * 1000
                Write-Host ("    Python (loop): {0,8:F1}ms" -f $pyMs) -ForegroundColor Blue
                if ($quinMs) {
                    $ratio = $pyMs / $quinMs
                    Write-Host ("               -> Quin is {0:F0}x faster than Python" -f $ratio) -ForegroundColor Green
                }
            }
        }

        # Node.js
        if ($jsBench) {
            $jsOut = (& node $jsBench.FullName 2>&1) -join "`n"
            if ($jsOut -match "JS Time:\s*([\d.]+)s") {
                $jsMs = [double]$matches[1] * 1000
                Write-Host ("    Node   (loop): {0,8:F1}ms" -f $jsMs) -ForegroundColor Yellow
                if ($quinMs) {
                    if ($quinMs -le $jsMs) {
                        $ratio = $jsMs / $quinMs
                        Write-Host ("               -> Quin is {0:F1}x faster than Node" -f $ratio) -ForegroundColor Green
                    } else {
                        $ratio = $quinMs / $jsMs
                        Write-Host ("               -> Node is {0:F1}x faster than Quin" -f $ratio) -ForegroundColor DarkYellow
                    }
                }
            }
        }
    }

    Write-Host ""
}

if ($Failed -gt 0) { exit 1 } else { exit 0 }