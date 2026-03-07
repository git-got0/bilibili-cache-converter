@echo off
echo ========================================
echo FFmpeg 下载脚本
echo ========================================
echo.
echo 正在从国内镜像下载 FFmpeg...
echo.

powershell -Command "& {[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://objects.githubusercontent.com/github-production-release-asset-2e65be/380323518/ffmpeg-master-latest-win64-gpl.zip' -OutFile 'ffmpeg.zip' -UseBasicParsing}"

if exist ffmpeg.zip (
    echo.
    echo 下载完成，正在解压...
    powershell -Command "Expand-Archive -Force ffmpeg.zip ."
    echo.
    echo 解压完成！
    echo 请将解压后的 ffmpeg 目录添加到系统 PATH
    echo 或将 ffmpeg.exe 所在路径配置到程序中
    echo.
    del ffmpeg.zip
) else (
    echo.
    echo 下载失败，请检查网络后重试
    echo 或者手动下载: https://github.com/BtbN/FFmpeg-Builds/releases
)

pause
