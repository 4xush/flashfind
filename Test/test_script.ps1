# FlashFind MVP Test Script
Write-Host "=== FlashFind MVP Test ===" -ForegroundColor Cyan

# 1. Build the application
Write-Host "Building application..." -ForegroundColor Yellow
cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# 2. Generate test data
Write-Host "`nCreating test data..." -ForegroundColor Yellow
$testDir = ".\TestData"
if (Test-Path $testDir) { Remove-Item $testDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $testDir

# Create 1,000 test files (reduced from 10,000 for speed)
1..1000 | ForEach-Object {
    $prefix = @("document", "image", "video", "archive", "code") | Get-Random
    $number = Get-Random -Minimum 1000 -Maximum 9999
    $ext = @(".txt", ".pdf", ".jpg", ".mp4", ".zip") | Get-Random
    
    $filename = "$($prefix)_$($number)_testfile$($ext)"
    $path = Join-Path $testDir $filename
    
    # Create 1KB file
    [System.IO.File]::WriteAllBytes($path, (New-Object byte[] 1024))
}

Write-Host "Created 1,000 test files in $testDir" -ForegroundColor Green

# 3. Run the application
Write-Host "`nStarting FlashFind..." -ForegroundColor Yellow
Start-Process -FilePath ".\target\release\flashfind-mvp.exe" -NoNewWindow

Write-Host "`nTest complete! Check the application window." -ForegroundColor Green