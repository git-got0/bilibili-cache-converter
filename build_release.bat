@echo off
cd /d d:\workspace-office-automatic\src-tauri
cargo build --release
if %errorlevel% equ 0 (
    echo Build successful!
    echo Output: d:\workspace-office-automatic\src-tauri\target\release\bilibili-converter.exe
) else (
    echo Build failed with error code: %errorlevel%
)
pause
