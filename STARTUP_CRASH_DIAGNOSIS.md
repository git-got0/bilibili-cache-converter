# 程序启动闪退诊断分析报告

## 一、问题现象描述

```
┌─────────────────────────────────────────────────────────────┐
│                    问题现象                                  │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. 程序启动时立即闪退                                        │
│  2. 后台进程持续加载（可能存在僵尸进程）                      │
│  3. 窗口短暂出现或不出现即消失                                │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 二、潜在原因分类与诊断

### 2.1 原因分类总览

```
┌─────────────────────────────────────────────────────────────┐
│                  启动闪退原因分类                            │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐        │
│  │  环境依赖   │   │  配置错误   │   │  权限问题   │        │
│  │  (5项)     │   │  (4项)     │   │  (3项)     │        │
│  └──────┬──────┘   └──────┬──────┘   └──────┬──────┘        │
│         │                 │                 │                │
│  ┌──────┴─────────────────┴─────────────────┴──────┐        │
│  │                运行时错误                       │        │
│  └──────────────────────┬──────────────────────────┘        │
│                         │                                    │
│  ┌─────────────┐   ┌────┴────┐   ┌─────────────┐            │
│  │  初始化失败 │   │ 内存问题 │   │  前端加载   │            │
│  │  (4项)     │   │  (3项)   │   │  (3项)     │            │
│  └─────────────┘   └─────────┘   └─────────────┘            │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 三、详细原因分析与排查步骤

### 3.1 环境依赖问题

#### 🔴 问题 A1: FFmpeg 未安装或路径不正确

**症状**: 程序启动正常但功能不可用，或启动时检查失败

**根本原因**:
```rust
// src-tauri/src/converter.rs 中的 FFmpeg 调用
let ffmpeg_path = get_ffmpeg_path(); // 如果 FFmpeg 不存在，可能返回无效路径
```

**排查步骤**:
```powershell
# 1. 检查 FFmpeg 是否安装
ffmpeg -version

# 2. 检查 FFmpeg 路径
where ffmpeg

# 3. 检查环境变量
echo $env:PATH
```

**解决方案**:
```powershell
# 方案1: 安装 FFmpeg
winget install ffmpeg

# 方案2: 下载并配置
# 下载 https://www.gyan.dev/ffmpeg/builds/
# 解压到 C:\ffmpeg
# 添加 C:\ffmpeg\bin 到系统 PATH

# 方案3: 使用项目自带的下载脚本
cd src-tauri/resources
.\download-ffmpeg.bat
```

---

#### 🔴 问题 A2: WebView2 运行时缺失 (Windows)

**症状**: 窗口无法创建，立即崩溃

**根本原因**: Tauri 依赖 WebView2 运行时渲染前端界面

**排查步骤**:
```powershell
# 检查 WebView2 是否安装
Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue

# 或检查注册表
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv
```

**解决方案**:
```powershell
# 下载并安装 WebView2
# https://developer.microsoft.com/en-us/microsoft-edge/webview2/

# 或使用 winget
winget install Microsoft.EdgeWebView2Runtime
```

---

#### 🟡 问题 A3: Visual C++ 运行库缺失

**症状**: 提示缺少 DLL 文件

**排查步骤**:
```powershell
# 检查 VC++ 运行库
Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" -ErrorAction SilentlyContinue
```

**解决方案**:
```powershell
# 安装 Visual C++ Redistributable
winget install Microsoft.VCRedist.2015+.x64
```

---

#### 🟡 问题 A4: Rust 编译目标不匹配

**症状**: 特定系统上无法启动

**排查步骤**:
```powershell
# 检查编译目标
rustup target list --installed

# 检查当前工具链
rustup show
```

**解决方案**:
```powershell
# 添加 Windows 目标
rustup target add x86_64-pc-windows-msvc

# 重新构建
cargo build --release
```

---

#### 🟡 问题 A5: Tauri 插件初始化失败

**症状**: 插件加载阶段崩溃

**相关代码位置**: `src-tauri/src/lib.rs:879-883`
```rust
.plugin(tauri_plugin_dialog::init())
.plugin(tauri_plugin_notification::init())
.plugin(tauri_plugin_shell::init())
.plugin(tauri_plugin_fs::init())
.plugin(tauri_plugin_os::init())
```

**排查步骤**:
```powershell
# 检查 Tauri 版本兼容性
cargo tree | findstr tauri

# 检查插件依赖
cargo tree | findstr tauri-plugin
```

**解决方案**:
```toml
# 确保 Cargo.toml 中的版本一致
[dependencies]
tauri = { version = "2.10.0", features = ["tray-icon"] }
tauri-plugin-log = "2"
tauri-plugin-dialog = "2"
tauri-plugin-notification = "2"
tauri-plugin-shell = "2"
tauri-plugin-fs = "2"
tauri-plugin-os = "2"
```

---

### 3.2 配置错误问题

#### 🔴 问题 B1: tauri.conf.json 配置错误

**症状**: 应用启动时崩溃

**关键配置检查**: `src-tauri/tauri.conf.json`

```json
// 检查以下配置项是否正确
{
  "build": {
    "frontendDist": "../dist",  // 确保此目录存在
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "windows": [{
      "title": "Bilibili缓存转换器",
      // 确保图标路径正确
    }],
    "trayIcon": {
      "iconPath": "icons/icon.ico"  // 确保图标文件存在
    }
  },
  "bundle": {
    "resources": {
      "resources/*": "./"  // 确保资源目录存在
    }
  }
}
```

**排查步骤**:
```powershell
# 1. 检查前端构建产物
Test-Path .\dist\index.html

# 2. 检查图标文件
Test-Path .\src-tauri\icons\icon.ico

# 3. 检查资源目录
Test-Path .\src-tauri\resources\

# 4. 验证 JSON 格式
Get-Content .\src-tauri\tauri.conf.json | ConvertFrom-Json
```

**解决方案**:
```powershell
# 1. 重新构建前端
npm run build

# 2. 确保图标存在
# 如果缺少图标，使用 tauri 生成
npx tauri icon /path/to/source-image.png

# 3. 创建资源目录
New-Item -ItemType Directory -Force -Path .\src-tauri\resources
```

---

#### 🟡 问题 B2: CSP (内容安全策略) 阻止

**症状**: 前端资源加载失败，控制台报错

**相关配置**: `src-tauri/tauri.conf.json:33`
```json
"security": {
  "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; img-src 'self' data: asset: https://asset.localhost"
}
```

**排查步骤**:
- 在开发模式下查看 DevTools 控制台
- 检查是否有 CSP 相关错误

**解决方案**:
```json
// 放宽 CSP 策略（仅用于调试）
"security": {
  "csp": "default-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: asset: https: http:; script-src 'self' 'unsafe-inline' 'unsafe-eval'"
}
```

---

#### 🟡 问题 B3: capabilities 权限配置不完整

**症状**: API 调用被拒绝

**相关文件**: `src-tauri/capabilities/default.json`

**排查步骤**:
```powershell
# 检查权限配置
Get-Content .\src-tauri\capabilities\default.json
```

**解决方案**:
```json
{
  "identifier": "default",
  "permissions": [
    "core:default",
    "core:path:default",
    "core:event:default",
    "core:window:default",
    "dialog:default",
    "dialog:allow-open",
    "dialog:allow-save",
    "fs:default",
    "fs:allow-read-file",
    "fs:allow-write-file",
    "fs:allow-read-dir",
    "fs:allow-mkdir",
    "shell:default",
    "shell:allow-open",
    "notification:default"
  ]
}
```

---

#### 🟡 问题 B4: 前端构建配置错误

**症状**: 白屏或资源加载失败

**排查步骤**:
```powershell
# 1. 检查 Vite 配置
Get-Content .\vite.config.ts

# 2. 检查 TypeScript 配置
Get-Content .\tsconfig.json

# 3. 验证构建产物
npm run build
Get-ChildItem .\dist
```

**解决方案**:
```typescript
// vite.config.ts 确保正确配置
export default defineConfig({
  plugins: [react()],
  base: './',  // 使用相对路径
  build: {
    target: 'chrome105',
    outDir: 'dist',
    emptyOutDir: true,
  },
  // ...
})
```

---

### 3.3 权限问题

#### 🔴 问题 C1: 应用数据目录无写入权限

**症状**: 日志初始化失败，应用崩溃

**相关代码**: `src-tauri/src/lib.rs:917-925`
```rust
let default_log_dir = app.path().app_data_dir().unwrap_or_else(|e| {
    eprintln!("Failed to get app data dir: {}", e);
    std::path::PathBuf::from(".")
});

if let Err(e) = std::fs::create_dir_all(&default_log_dir) {
    eprintln!("Failed to create default log dir: {}", e);
}
```

**排查步骤**:
```powershell
# 检查 AppData 目录权限
$env:APPDATA
icacls "$env:APPDATA\com.bilibili.converter"

# 检查当前用户权限
whoami /all
```

**解决方案**:
```powershell
# 以管理员身份运行
# 或修复目录权限
$appData = "$env:APPDATA\com.bilibili.converter"
New-Item -ItemType Directory -Force -Path $appData
icacls $appData /grant "${env:USERNAME}:(OI)(CI)F"
```

---

#### 🟡 问题 C2: 防病毒软件拦截

**症状**: 程序被隔离或阻止运行

**排查步骤**:
```powershell
# 检查 Windows Defender 隔离
Get-MpThreatDetection | Select-Object -First 10

# 检查排除列表
Get-MpPreference | Select-Object ExclusionPath, ExclusionProcess
```

**解决方案**:
```powershell
# 添加排除项（管理员权限）
Add-MpPreference -ExclusionPath "D:\workspace-office-automatic"
Add-MpPreference -ExclusionProcess "bilibili-converter.exe"
```

---

#### 🟡 问题 C3: 安装目录权限不足

**症状**: 无法创建必要文件

**排查步骤**:
```powershell
# 检查安装目录权限
icacls "C:\Program Files\Bilibili缓存转换器"
```

**解决方案**:
- 安装到用户目录而非 Program Files
- 或以管理员权限运行

---

### 3.4 运行时初始化失败

#### 🔴 问题 D1: 日志系统初始化失败

**症状**: 启动时立即崩溃

**相关代码**: `src-tauri/src/logger.rs:122-141`
```rust
pub fn init_logger(config: LoggerConfig) -> Result<(), String> {
    if let Err(e) = fs::create_dir_all(&config.log_dir) {
        return Err(format!("Failed to create log directory: {}", e));
    }
    
    let mut state = LOGGER.lock().map_err(|e| format!("Logger lock error: {}", e))?;
    // ...
}
```

**问题分析**:
1. 日志目录创建失败
2. 全局锁获取失败
3. 文件句柄耗尽

**排查步骤**:
```powershell
# 检查日志目录
$appLogDir = "$env:APPDATA\com.bilibili.converter\logs"
Test-Path $appLogDir

# 检查磁盘空间
Get-PSDrive C | Select-Object Used, Free

# 检查文件句柄
handle64.exe -p bilibili-converter  # 需要 Sysinternals Suite
```

---

#### 🔴 问题 D2: 托盘图标创建失败

**症状**: 托盘图标创建时崩溃

**相关代码**: `src-tauri/src/lib.rs:949-982`
```rust
let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
let menu = Menu::with_items(app, &[&show, &quit])?;

let _tray = TrayIconBuilder::new()
    .menu(&menu)
    .tooltip("Bilibili缓存转换器")
    // ...
    .build(app)?;
```

**排查步骤**:
```powershell
# 检查图标文件
Test-Path .\src-tauri\icons\icon.ico

# 检查图标格式
# ICO 文件应该是有效的 Windows 图标格式
```

**解决方案**:
```powershell
# 重新生成图标
# 准备一个 PNG 图片，运行：
npx tauri icon source-image.png

# 或手动检查 ICO 文件完整性
```

---

#### 🟡 问题 D3: 窗口创建失败

**症状**: 托盘图标存在但窗口不显示

**相关代码**: `src-tauri/tauri.conf.json:13-26`
```json
"windows": [{
  "title": "Bilibili缓存转换器",
  "width": 900,
  "height": 650,
  // ...
}]
```

**排查步骤**:
```powershell
# 检查窗口配置
# 尝试创建更小的窗口

# 检查显示器分辨率
Get-WmiObject -Class Win32_DesktopMonitor | Select-Object ScreenWidth, ScreenHeight
```

---

#### 🟡 问题 D4: 插件依赖缺失

**症状**: 特定插件初始化时崩溃

**排查步骤**:
```powershell
# 检查 Cargo.lock 确保依赖完整
cargo check

# 运行时检查
cargo run --features=tray-icon
```

---

### 3.5 内存与资源问题

#### 🔴 问题 E1: 内存不足

**症状**: 启动时内存分配失败

**排查步骤**:
```powershell
# 检查系统内存
Get-WmiObject Win32_OperatingSystem | Select-Object FreePhysicalMemory, TotalVisibleMemorySize

# 检查进程内存使用
Get-Process | Sort-Object WorkingSet64 -Descending | Select-Object -First 10 Name, @{N='Memory(MB)';E={[math]::Round($_.WorkingSet64/1MB,2)}}
```

**解决方案**:
- 关闭其他内存密集型应用
- 增加虚拟内存

---

#### 🟡 问题 E2: 端口冲突

**症状**: 开发模式下无法启动

**相关配置**: `vite.config.ts` 和 `tauri.conf.json`
```json
"devUrl": "http://localhost:5173"
```

**排查步骤**:
```powershell
# 检查端口占用
netstat -ano | findstr :5173
netstat -ano | findstr :5174

# 检查进程
Get-Process -Id (Get-NetTCPConnection -LocalPort 5173).OwningProcess -ErrorAction SilentlyContinue
```

**解决方案**:
```powershell
# 终止占用端口的进程
Stop-Process -Id <PID> -Force

# 或修改配置使用其他端口
```

---

#### 🟡 问题 E3: 文件句柄耗尽

**症状**: 无法打开新文件

**排查步骤**:
```powershell
# 检查打开的文件句柄数
# 需要使用 Sysinternals Suite 的 handle64
handle64.exe -p bilibili-converter | Measure-Object

# 检查系统限制
# Windows 默认句柄限制很高，通常不会耗尽
```

---

### 3.6 前端加载问题

#### 🔴 问题 F1: 前端资源加载失败

**症状**: 白屏或显示错误

**相关代码**: `index.html:61-67`
```html
<div id="loading-screen">
  <div style="text-align: center;">
    <div class="loading-spinner"></div>
    <div class="loading-text">正在加载...</div>
  </div>
</div>
<div id="root"></div>
<script type="module" src="/src/main.tsx"></script>
```

**排查步骤**:
```powershell
# 检查构建产物完整性
Get-ChildItem .\dist -Recurse

# 检查 index.html
Get-Content .\dist\index.html
```

**解决方案**:
```powershell
# 重新构建前端
npm run build

# 清理缓存后重建
Remove-Item -Recurse -Force .\dist, .\node_modules\.vite
npm run build
```

---

#### 🔴 问题 F2: React 初始化错误

**症状**: JavaScript 运行时错误

**相关代码**: `src/main.tsx:7-12`
```typescript
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
    <Toaster position="bottom-right" richColors />
  </StrictMode>,
)
```

**排查步骤**:
```powershell
# 开发模式下查看控制台错误
npm run tauri:dev

# 检查 TypeScript 编译错误
npx tsc --noEmit
```

---

#### 🟡 问题 F3: CSS 样式加载失败

**症状**: 界面显示异常

**排查步骤**:
```powershell
# 检查 CSS 文件
Get-ChildItem .\dist\assets\*.css

# 检查 Tailwind 配置
Get-Content .\tailwind.config.js
```

---

## 四、诊断流程图

```
┌─────────────────────────────────────────────────────────────┐
│                    启动故障诊断流程                          │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  开始                                                         │
│    │                                                          │
│    ▼                                                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ 1. 检查是否有错误日志                                │    │
│  │    位置: %APPDATA%\com.bilibili.converter\logs\     │    │
│  └──────────────────────┬──────────────────────────────┘    │
│                         │                                    │
│           ┌─────────────┴─────────────┐                     │
│           │                           │                      │
│    有日志  ▼                     无日志  ▼                    │
│  ┌─────────────────┐          ┌─────────────────┐           │
│  │ 分析日志内容     │          │ 检查环境依赖    │           │
│  │ 定位崩溃点       │          │ • WebView2      │           │
│  └────────┬────────┘          │ • VC++ 运行库   │           │
│           │                    │ • FFmpeg        │           │
│           │                    └────────┬────────┘           │
│           │                             │                    │
│           └──────────┬──────────────────┘                    │
│                      │                                       │
│                      ▼                                       │
│           ┌─────────────────────────────────────┐           │
│           │ 2. 检查配置文件                       │           │
│           │ • tauri.conf.json                    │           │
│           │ • capabilities/default.json          │           │
│           │ • vite.config.ts                     │           │
│           └──────────────────┬──────────────────┘           │
│                              │                               │
│                              ▼                               │
│           ┌─────────────────────────────────────┐           │
│           │ 3. 检查构建产物                       │           │
│           │ • dist/ 目录                         │           │
│           │ • 图标文件                           │           │
│           │ • 资源文件                           │           │
│           └──────────────────┬──────────────────┘           │
│                              │                               │
│                              ▼                               │
│           ┌─────────────────────────────────────┐           │
│           │ 4. 尝试开发模式运行                   │           │
│           │ npm run tauri:dev                    │           │
│           │ 查看详细错误信息                      │           │
│           └──────────────────┬──────────────────┘           │
│                              │                               │
│                              ▼                               │
│           ┌─────────────────────────────────────┐           │
│           │ 5. 检查权限和安全软件                 │           │
│           │ • 防病毒软件                         │           │
│           │ • 目录权限                           │           │
│           │ • 管理员权限                         │           │
│           └──────────────────┬──────────────────┘           │
│                              │                               │
│                              ▼                               │
│                         问题定位                              │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 五、快速诊断脚本

### 5.1 PowerShell 诊断脚本

```powershell
# 保存为 diagnose.ps1
# 以管理员身份运行

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Bilibili缓存转换器 - 启动诊断工具" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 1. 检查 WebView2
Write-Host "[1/8] 检查 WebView2 运行时..." -ForegroundColor Yellow
$webview2 = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
if ($webview2) {
    Write-Host "  ✓ WebView2 已安装: $($webview2.pv)" -ForegroundColor Green
} else {
    Write-Host "  ✗ WebView2 未安装!" -ForegroundColor Red
    Write-Host "    解决: winget install Microsoft.EdgeWebView2Runtime" -ForegroundColor Yellow
}

# 2. 检查 FFmpeg
Write-Host "[2/8] 检查 FFmpeg..." -ForegroundColor Yellow
$ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
if ($ffmpeg) {
    $version = (ffmpeg -version 2>&1 | Select-Object -First 1) -replace "ffmpeg version ", ""
    Write-Host "  ✓ FFmpeg 已安装: $version" -ForegroundColor Green
} else {
    Write-Host "  ✗ FFmpeg 未找到!" -ForegroundColor Red
    Write-Host "    解决: winget install ffmpeg" -ForegroundColor Yellow
}

# 3. 检查 VC++ 运行库
Write-Host "[3/8] 检查 Visual C++ 运行库..." -ForegroundColor Yellow
$vc = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" -ErrorAction SilentlyContinue
if ($vc) {
    Write-Host "  ✓ VC++ 运行库已安装: $($vc.Version)" -ForegroundColor Green
} else {
    Write-Host "  ✗ VC++ 运行库未安装!" -ForegroundColor Red
    Write-Host "    解决: winget install Microsoft.VCRedist.2015+.x64" -ForegroundColor Yellow
}

# 4. 检查构建产物
Write-Host "[4/8] 检查构建产物..." -ForegroundColor Yellow
if (Test-Path ".\dist\index.html") {
    Write-Host "  ✓ 前端构建产物存在" -ForegroundColor Green
} else {
    Write-Host "  ✗ 前端构建产物缺失!" -ForegroundColor Red
    Write-Host "    解决: npm run build" -ForegroundColor Yellow
}

# 5. 检查图标文件
Write-Host "[5/8] 检查图标文件..." -ForegroundColor Yellow
$icons = @("icons\32x32.png", "icons\128x128.png", "icons\icon.ico")
foreach ($icon in $icons) {
    $iconPath = ".\src-tauri\$icon"
    if (Test-Path $iconPath) {
        Write-Host "  ✓ $icon 存在" -ForegroundColor Green
    } else {
        Write-Host "  ✗ $icon 缺失!" -ForegroundColor Red
    }
}

# 6. 检查配置文件
Write-Host "[6/8] 检查配置文件..." -ForegroundColor Yellow
$configs = @(
    ".\src-tauri\tauri.conf.json",
    ".\src-tauri\capabilities\default.json",
    ".\vite.config.ts",
    ".\tsconfig.json"
)
foreach ($config in $configs) {
    if (Test-Path $config) {
        try {
            if ($config -match "\.json$") {
                Get-Content $config | ConvertFrom-Json | Out-Null
            }
            Write-Host "  ✓ $config 有效" -ForegroundColor Green
        } catch {
            Write-Host "  ✗ $config 格式错误: $($_.Exception.Message)" -ForegroundColor Red
        }
    } else {
        Write-Host "  ✗ $config 不存在!" -ForegroundColor Red
    }
}

# 7. 检查端口占用
Write-Host "[7/8] 检查开发端口..." -ForegroundColor Yellow
$port = Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
if ($port) {
    Write-Host "  ! 端口 5173 已被占用" -ForegroundColor Yellow
    Write-Host "    PID: $($port.OwningProcess)" -ForegroundColor Yellow
} else {
    Write-Host "  ✓ 端口 5173 可用" -ForegroundColor Green
}

# 8. 检查日志目录
Write-Host "[8/8] 检查日志目录..." -ForegroundColor Yellow
$logDir = "$env:APPDATA\com.bilibili.converter\logs"
if (Test-Path $logDir) {
    $logs = Get-ChildItem $logDir -Filter "*.log" | Sort-Object LastWriteTime -Descending | Select-Object -First 3
    Write-Host "  ✓ 日志目录存在" -ForegroundColor Green
    if ($logs) {
        Write-Host "    最近日志文件:" -ForegroundColor Gray
        foreach ($log in $logs) {
            Write-Host "      - $($log.Name)" -ForegroundColor Gray
        }
    }
} else {
    Write-Host "  ! 日志目录不存在 (首次运行时创建)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "诊断完成" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

# 如果发现问题，给出建议
Write-Host ""
Write-Host "下一步操作建议:" -ForegroundColor White
Write-Host "1. 如有问题，按上述提示修复" -ForegroundColor White
Write-Host "2. 尝试开发模式: npm run tauri:dev" -ForegroundColor White
Write-Host "3. 查看日志: $logDir" -ForegroundColor White
Write-Host "4. 重新构建: npm run build && cargo build --release" -ForegroundColor White
```

---

## 六、常见问题速查表

| 症状 | 最可能原因 | 解决方案 |
|------|-----------|----------|
| 双击无反应 | WebView2缺失 | 安装 WebView2 Runtime |
| 白屏后退出 | 前端构建失败 | `npm run build` |
| 提示缺少DLL | VC++运行库缺失 | 安装 VC++ Redistributable |
| 托盘图标创建失败 | 图标文件损坏 | 重新生成图标 |
| 权限被拒绝 | 防病毒软件拦截 | 添加排除项 |
| 端口被占用 | 开发服务冲突 | 终止占用进程 |
| 日志目录错误 | 权限不足 | 以管理员运行 |
| 内存不足 | 系统资源耗尽 | 关闭其他程序 |

---

## 七、日志分析指南

### 7.1 日志位置

```
Windows: %APPDATA%\com.bilibili.converter\logs\
macOS: ~/Library/Application Support/com.bilibili.converter/logs/
Linux: ~/.config/com.bilibili.converter/logs/
```

### 7.2 关键错误模式

```
# 日志初始化失败
"Failed to create log directory"
"Logger lock error"

# 托盘创建失败
"Failed to create tray icon"
"Icon path not found"

# WebView 错误
"WebView initialization failed"
"Failed to create webview"

# 插件加载失败
"Failed to initialize plugin"
```

### 7.3 日志查看命令

```powershell
# 查看最新日志
Get-Content "$env:APPDATA\com.bilibili.converter\logs\app_*.log" -Tail 50

# 搜索错误
Select-String -Path "$env:APPDATA\com.bilibili.converter\logs\*.log" -Pattern "ERROR|FATAL|panic"

# 实时监控日志
Get-Content "$env:APPDATA\com.bilibili.converter\logs\app_*.log" -Wait
```

---

## 八、后台进程持续问题

### 8.1 问题分析

```
┌─────────────────────────────────────────────────────────────┐
│                  后台进程持续原因                            │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  1. 托盘图标模式                                             │
│     └─ 关闭窗口时程序不退出，继续在托盘运行                  │
│                                                               │
│  2. 后台转换任务                                             │
│     └─ 转换任务在独立线程运行，窗口关闭不中断                │
│                                                               │
│  3. FFmpeg 进程残留                                          │
│     └─ 转换崩溃时 FFmpeg 子进程未被清理                      │
│                                                               │
│  4. 僵尸进程                                                 │
│     └─ 主进程崩溃但子进程仍在运行                            │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 8.2 进程清理脚本

```powershell
# 保存为 cleanup.ps1

Write-Host "清理残留进程..." -ForegroundColor Yellow

# 1. 终止主程序
Get-Process -Name "bilibili-converter" -ErrorAction SilentlyContinue | Stop-Process -Force

# 2. 终止 FFmpeg 进程
Get-Process -Name "ffmpeg" -ErrorAction SilentlyContinue | Stop-Process -Force

# 3. 终止相关 Tauri 进程
Get-Process | Where-Object { $_.MainWindowTitle -like "*Bilibili*" } | Stop-Process -Force

# 4. 清理僵尸进程
Get-Process | Where-Object { $_.Responding -eq $false } | Stop-Process -Force

Write-Host "清理完成" -ForegroundColor Green

# 显示当前进程
Write-Host "`n当前相关进程:" -ForegroundColor Cyan
Get-Process | Where-Object { $_.Name -match "bilibili|ffmpeg|tauri" }
```

---

## 九、预防措施

### 9.1 开发阶段

1. **完善的错误处理**
```rust
// 在 run() 函数中添加更详细的错误信息
pub fn run() {
    // 设置 panic hook
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("FATAL: {}", info);
        eprintln!("{}", msg);
        
        // 尝试写入错误日志
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("crash.log")
        {
            use std::io::Write;
            let _ = writeln!(file, "[{}] {}", chrono_lite_timestamp(), msg);
        }
    }));
    
    // 原有启动逻辑...
}
```

2. **优雅降级**
```rust
// 托盘创建失败时不崩溃
let _tray = TrayIconBuilder::new()
    // ...
    .build(app)
    .map_err(|e| {
        log::error!("Tray icon creation failed: {}", e);
        // 不返回错误，允许程序继续运行
        e
    })
    .ok();
```

### 9.2 部署阶段

1. **安装必要依赖**
```nsis
; NSIS 安装脚本
Section "Prerequisites" SEC01
    ; 检查 WebView2
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
    StrCmp $0 "" 0 webview2_installed
    
    ; 安装 WebView2
    File "webview2-bootstrapper.exe"
    ExecWait '"$INSTDIR\webview2-bootstrapper.exe" /silent'
    
webview2_installed:
SectionEnd
```

2. **启动前检查**
```powershell
# 启动脚本
$webview2 = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
if (-not $webview2) {
    [System.Windows.Forms.MessageBox]::Show(
        "需要安装 WebView2 运行时。程序将自动下载安装。",
        "缺少依赖",
        "OK",
        "Warning"
    )
    Start-Process "https://developer.microsoft.com/en-us/microsoft-edge/webview2/"
    exit 1
}

# 启动主程序
Start-Process "bilibili-converter.exe"
```

---

## 十、总结

### 启动故障检查清单

```markdown
## 启动前检查
- [ ] WebView2 运行时已安装
- [ ] Visual C++ 运行库已安装
- [ ] FFmpeg 已安装（可选，运行时需要）

## 构建检查
- [ ] 前端构建产物完整 (dist/)
- [ ] 图标文件存在
- [ ] 资源文件存在
- [ ] 配置文件格式正确

## 权限检查
- [ ] 应用数据目录可写
- [ ] 安装目录有访问权限
- [ ] 未被防病毒软件隔离

## 运行时检查
- [ ] 开发模式可正常运行
- [ ] 无端口冲突
- [ ] 日志可正常写入
```

### 紧急修复步骤

```
1. 运行诊断脚本 (diagnose.ps1)
2. 根据提示修复缺失依赖
3. 清理并重新构建 (npm run build && cargo build --release)
4. 检查日志文件定位具体错误
5. 以管理员身份运行或调整权限
6. 添加防病毒软件排除项
```

---

**文档版本**: 1.0  
**创建日期**: 2026-03-11  
**适用版本**: Bilibili缓存转换器 v1.0.0
