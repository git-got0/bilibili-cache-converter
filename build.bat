@echo off
echo ========================================
echo Bilibili Cache Converter - Full Build Script
echo ========================================
echo.

echo Step 1/2: Building frontend...
npm run build
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Frontend build failed!
    pause
    exit /b 1
)

echo.
echo Step 2/2: Building executable...
npm run tauri:build
if %ERRORLEVEL% EQU 0 (
    echo.
    echo ========================================
    echo Build successful!
    echo ========================================
    echo.
    echo Executable location:
    echo   src-tauri\target\release\bilibili-converter.exe
    echo.
    echo Installer location:
    echo   src-tauri\target\release\bundle\nis\Bilibili_Cache_Converter_1.0.0_x64-setup.exe
    echo ========================================
) else (
    echo.
    echo ========================================
    echo Build failed (Error code: %ERRORLEVEL%)
    echo ========================================
)

pause
