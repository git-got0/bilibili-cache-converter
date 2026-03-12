@echo off
echo ========================================
echo Bilibili Cache Converter - Full Build
echo ========================================
echo.

echo Step 1/2: Building frontend...
cd /d d:\workspace-office-automatic
call npm run build
if %errorlevel% neq 0 (
    echo Frontend build failed!
    pause
    exit /b 1
)
echo Frontend build complete!
echo.

echo Step 2/2: Building Rust backend...
cd /d d:\workspace-office-automatic\src-tauri
cargo build --release
if %errorlevel% neq 0 (
    echo Rust build failed!
    pause
    exit /b 1
)
echo Rust build complete!
echo.

echo ========================================
echo Build Successfully Completed!
echo ========================================
echo.
echo Executable: d:\workspace-office-automatic\src-tauri\target\release\bilibili-converter.exe
echo.
pause
