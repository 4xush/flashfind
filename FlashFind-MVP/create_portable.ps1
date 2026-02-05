# Create Portable Distribution Package
# Run this to create a ready-to-distribute ZIP

$version = "1.0.0"
$appName = "FlashFind"
$distDir = "dist\FlashFind-v$version-Windows-Portable"

Write-Host "Creating portable distribution package..." -ForegroundColor Cyan

# Clean and create dist folder
if (Test-Path dist) { Remove-Item dist -Recurse -Force }
New-Item -ItemType Directory -Path $distDir -Force | Out-Null

# Copy exe
Copy-Item "target\release\flashfind.exe" "$distDir\FlashFind.exe"

# Create README
@"
# FlashFind v$version - Portable Edition

## Quick Start
1. Double-click **FlashFind.exe** to launch
2. The app will start indexing your files automatically
3. Start typing to search instantly

## First Launch
- FlashFind indexes C: drive user folders (Documents, Downloads, Desktop, etc.)
- Indexing happens in the background
- Search works immediately even during indexing

## Features
- Lightning-fast file search (search millions of files in milliseconds)
- Real-time monitoring (index updates as files change)
- Filter by file type (documents, images, videos, code, etc.)
- Export results to CSV
- Keyboard shortcuts: Enter to open, Esc to clear

## Settings
Click **⚙ Settings** to:
- Change theme (Dark/Light/System)
- Adjust auto-save interval
- View index statistics
- Compact index to free memory

## Data Storage
- Index: `%APPDATA%\flashfind\index.bin`
- Config: `%APPDATA%\flashfind\config.json`
- Logs: `%APPDATA%\flashfind\logs\`

## System Requirements
- Windows 7 SP1 or later (64-bit)
- ~50MB RAM while idle, ~200MB during indexing
- ~10-100MB disk space for index (depends on file count)

## Uninstall
1. Close FlashFind
2. Delete FlashFind.exe
3. (Optional) Delete `%APPDATA%\flashfind` to remove all data

## Support
Report issues: https://github.com/4xush/flashfind/issues

---
© 2026 FlashFind | MIT License
"@ | Out-File -FilePath "$distDir\README.txt" -Encoding UTF8

# Create LICENSE
@"
MIT License

Copyright (c) 2026 Ayush Kumar

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"@ | Out-File -FilePath "$distDir\LICENSE.txt" -Encoding UTF8

# Create ZIP
$zipPath = "dist\FlashFind-v$version-Windows-Portable.zip"
Write-Host "Creating ZIP archive..." -ForegroundColor Yellow
Compress-Archive -Path $distDir -DestinationPath $zipPath -Force

$size = [math]::Round((Get-Item $zipPath).Length/1MB, 2)

Write-Host ""
Write-Host "Success! Portable package created!" -ForegroundColor Green
Write-Host ""
Write-Host "Package: $zipPath" -ForegroundColor Cyan
Write-Host "Size: $size MB" -ForegroundColor Cyan
Write-Host ""
Write-Host "Ready to distribute! Users just:" -ForegroundColor Yellow
Write-Host "  1. Extract ZIP"
Write-Host "  2. Run FlashFind.exe"
Write-Host ""
