@echo off
echo Building FlashFind MVP...
echo.

REM Clean previous build
echo Cleaning previous build...
cargo clean

REM Build release version
echo Building release version...
cargo build --release

if %ERRORLEVEL% NEQ 0 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo Build successful! Running FlashFind...
echo.

REM Run the application
.\target\release\flashfind-mvp.exe

pause