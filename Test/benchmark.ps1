# FlashFind vs Windows Explorer Benchmark
Write-Host "=== FlashFind Benchmark ===" -ForegroundColor Cyan

# Test queries
$testQueries = @(
    "*.txt",
    "*.pdf", 
    "document",
    "image",
    "*.exe",
    "2024",
    "report"
)

Write-Host "`n1. Testing FlashFind..." -ForegroundColor Yellow
foreach ($query in $testQueries) {
    # Note: You'll need to manually time FlashFind for now
    Write-Host "  Query: '$query'" -ForegroundColor Gray
}

Write-Host "`n2. Testing Windows Explorer..." -ForegroundColor Yellow
foreach ($query in $testQueries) {
    Write-Host "  Testing: '$query'" -ForegroundColor Gray
    
    # Time Windows search
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    
    if ($query -like "*.*") {
        # Extension search
        $ext = $query.TrimStart('*')
        $results = Get-ChildItem -Path $env:USERPROFILE -Filter "*$ext" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 100
    } else {
        # Name search
        $results = Get-ChildItem -Path $env:USERPROFILE -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "*$query*" } | Select-Object -First 100
    }
    
    $stopwatch.Stop()
    Write-Host "    Time: $($stopwatch.ElapsedMilliseconds)ms, Results: $($results.Count)" -ForegroundColor White
}

Write-Host "`n=== Instructions ===" -ForegroundColor Green
Write-Host "1. Run FlashFind and test the same queries"
Write-Host "2. Compare the search times"
Write-Host "3. FlashFind should be significantly faster!"