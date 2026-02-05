#!/usr/bin/env pwsh
# Quick manual test for FlashFind edge cases

Write-Host "FlashFind Quick Test" -ForegroundColor Cyan
Write-Host ""

$TestFile = "C:\Users\$env:USERNAME\Desktop\flashfind_test_$(Get-Date -Format 'HHmmss').txt"

Write-Host "1. Creating test file..." -ForegroundColor Yellow
"Test content $(Get-Date)" | Out-File -FilePath $TestFile
Write-Host "   Created: $TestFile" -ForegroundColor Green
Write-Host "   → Search for this file in FlashFind" -ForegroundColor White
Write-Host ""

Read-Host "Press Enter after you've found it in FlashFind"

Write-Host "`n2. Deleting test file..." -ForegroundColor Yellow
Remove-Item -Path $TestFile -Force
Write-Host "   Deleted: $TestFile" -ForegroundColor Green
Write-Host "   → File should disappear from FlashFind within seconds" -ForegroundColor White
Write-Host ""

Write-Host "✓ Test complete!" -ForegroundColor Green
