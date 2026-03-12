@echo off
echo ========================================
echo Bilibili Cache Converter - Installer Build
echo ========================================
echo.

cd /d d:\workspace-office-automatic
npm run tauri build

if %errorlevel% equ 0 (
    echo.
    echo ========================================
    echo Installer Build Successfully Completed!
    echo ========================================
    echo.
    echo Executable: d:\workspace-office-automatic\src-tauri\target\release\bilibili-converter.exe
    echo Installer: d:\workspace-office-automatic\src-tauri\target\release\bundle\nsis\Bilibili缓存转换器_1.0.0_x64-setup.exe
    echo.
) else (
    echo.
    echo Installer build failed with error code: %errorlevel%
    echo.
)

pause
