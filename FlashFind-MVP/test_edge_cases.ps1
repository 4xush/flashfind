# FlashFind Edge Case Testing Script
# Tests file creation, deletion, renaming, temp files, and permissions

Write-Host "=== FlashFind Edge Case Testing ===" -ForegroundColor Cyan
Write-Host ""

# Setup test directory
$TestDir = "C:\Users\$env:USERNAME\Desktop\FlashFind_Test"
Write-Host "[1] Creating test directory: $TestDir" -ForegroundColor Yellow

if (Test-Path $TestDir) {
    Remove-Item -Path $TestDir -Recurse -Force
}
New-Item -ItemType Directory -Path $TestDir | Out-Null

Write-Host "    ✓ Test directory created" -ForegroundColor Green
Start-Sleep -Seconds 2

# Test 1: Create a normal file
Write-Host "`n[2] Test: Create normal file" -ForegroundColor Yellow
$NormalFile = Join-Path $TestDir "test_document.txt"
"This is a test file" | Out-File -FilePath $NormalFile
Write-Host "    ✓ Created: $NormalFile" -ForegroundColor Green
Write-Host "    → Should appear in FlashFind search for 'test_document'" -ForegroundColor White
Start-Sleep -Seconds 3

# Test 2: Create temp files (should be ignored)
Write-Host "`n[3] Test: Create temp files (should be ignored)" -ForegroundColor Yellow
$TempFiles = @(
    "~`$temp_office.docx",
    "file.tmp",
    "download.part",
    "chrome.crdownload"
)

foreach ($temp in $TempFiles) {
    $tempPath = Join-Path $TestDir $temp
    "temp" | Out-File -FilePath $tempPath
    Write-Host "    ✓ Created temp: $temp" -ForegroundColor Green
}
Write-Host "    → These should NOT appear in FlashFind" -ForegroundColor White
Start-Sleep -Seconds 3

# Test 3: Delete the normal file
Write-Host "`n[4] Test: Delete normal file" -ForegroundColor Yellow
Remove-Item -Path $NormalFile -Force
Write-Host "    ✓ Deleted: test_document.txt" -ForegroundColor Green
Write-Host "    → Should disappear from FlashFind search" -ForegroundColor White
Start-Sleep -Seconds 3

# Test 4: Create and modify rapidly (stability check)
Write-Host "`n[5] Test: Rapid file changes (stability check)" -ForegroundColor Yellow
$RapidFile = Join-Path $TestDir "rapidly_changing.txt"
"Initial content" | Out-File -FilePath $RapidFile
for ($i = 1; $i -le 5; $i++) {
    "Update $i" | Add-Content -Path $RapidFile
    Start-Sleep -Milliseconds 50
}
Write-Host "    ✓ Created and modified file 5 times quickly" -ForegroundColor Green
Write-Host "    → FlashFind should wait until file is stable" -ForegroundColor White
Start-Sleep -Seconds 3

# Test 5: Large file simulation
Write-Host "`n[6] Test: Large file creation" -ForegroundColor Yellow
$LargeFile = Join-Path $TestDir "large_file.dat"
$stream = [System.IO.File]::Create($LargeFile)
try {
    # Write in chunks to simulate partial write
    for ($i = 0; $i -lt 10; $i++) {
        $data = [byte[]]::new(1MB)
        $stream.Write($data, 0, $data.Length)
        $stream.Flush()
        if ($i -eq 0) {
            Write-Host "    → Writing large file in chunks..." -ForegroundColor White
        }
        Start-Sleep -Milliseconds 100
    }
} finally {
    $stream.Close()
}
Write-Host "    ✓ Created 10MB file" -ForegroundColor Green
Write-Host "    → Should be indexed only when fully written" -ForegroundColor White
Start-Sleep -Seconds 3

# Test 6: Rename operation
Write-Host "`n[7] Test: File rename" -ForegroundColor Yellow
$OldName = $RapidFile
$NewName = Join-Path $TestDir "renamed_file.txt"
Rename-Item -Path $OldName -NewName "renamed_file.txt"
Write-Host "    ✓ Renamed: rapidly_changing.txt → renamed_file.txt" -ForegroundColor Green
Write-Host "    → Old name should disappear, new name should appear" -ForegroundColor White
Start-Sleep -Seconds 3

# Test 7: Permission test (read-only file)
Write-Host "`n[8] Test: Read-only file" -ForegroundColor Yellow
$ReadOnlyFile = Join-Path $TestDir "readonly.txt"
"Read-only content" | Out-File -FilePath $ReadOnlyFile
Set-ItemProperty -Path $ReadOnlyFile -Name IsReadOnly -Value $true
Write-Host "    ✓ Created read-only file" -ForegroundColor Green
Write-Host "    → Should still be indexed (we have read permission)" -ForegroundColor White
Start-Sleep -Seconds 3

# Summary
Write-Host "`n=== Test Summary ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Test Files Created:" -ForegroundColor Yellow
Write-Host "  • renamed_file.txt (should be indexed)"
Write-Host "  • large_file.dat (should be indexed)"
Write-Host "  • readonly.txt (should be indexed)"
Write-Host "  • 4 temp files (should NOT be indexed)"
Write-Host ""
Write-Host "Expected Behaviors:" -ForegroundColor Yellow
Write-Host "  1. Normal files appear in search immediately"
Write-Host "  2. Temp files are filtered and never appear"
Write-Host "  3. Deleted files disappear from search"
Write-Host "  4. Renamed files: old name gone, new name appears"
Write-Host "  5. Files being written are delayed until stable"
Write-Host "  6. Large files indexed only when complete"
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Green
Write-Host "  1. Open FlashFind and search for 'FlashFind_Test'"
Write-Host "  2. Verify only 3 files appear (renamed, large, readonly)"
Write-Host "  3. Try 'Compact Index' in Settings → Statistics"
Write-Host "  4. Delete test folder when done: Remove-Item '$TestDir' -Recurse -Force"
Write-Host ""
