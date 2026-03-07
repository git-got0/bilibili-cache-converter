# Bilibili缓存转换器

一款面向Windows系统的高性能桌面应用程序，用于将Bilibili缓存的音视频文件转换为通用格式。

## 功能特性

- **文件夹选择**：用户通过GUI选择目标文件夹
- **智能扫描**：自动识别Bilibili缓存音视频数据（.blv、.m4s等格式）
- **格式转换**：支持转换为MP4（视频）、MP3（音频）等通用格式
- **格式选项菜单**：提供多种输出格式供用户选择（视频：MP4/MKV/AVI，音频：MP3/AAC/FLAC）
- **实时监控**：界面实时显示运行状态、转换进度和完成情况
- **高性能显示**：支持虚拟滚动，高效渲染大量文件列表
- **高并发处理**：支持多线程并行转换，提升处理效率（1/2/4/8并发可选）
- **暂停/恢复**：支持暂停和恢复转换任务
- **系统托盘**：支持最小化到系统托盘运行
- **通知提醒**：任务完成后右下角弹出通知
- **设置选项**：支持设置提示音开关、并发数、输出路径等
- **性能监控**：显示实时转换速度、预计剩余时间、输出大小预估等性能指标

## 技术栈

- **桌面框架**：Tauri 2.x（Rust后端 + Web前端）
- **前端框架**：React 18 + TypeScript
- **UI组件库**：shadcn/ui + Tailwind CSS
- **音视频处理**：ffmpeg（需要用户安装）
- **并发处理**：Rust tokio多线程异步处理

## 环境要求

### 前端开发
- Node.js 18+
- npm 9+

### 后端编译
- Rust 1.77+
- Cargo

### 运行依赖
- FFmpeg（需要添加到系统PATH）

## 安装指南

### 1. 安装Rust（如果未安装）

#### Windows
```powershell
# 方法1：使用官方安装脚本
Invoke-WebRequest -Uri https://win.rustup.rs -OutFile rustup-init.exe
.\rustup-init.exe -y

# 方法2：使用Chocolatey（如果已安装）
choco install rust -y
```

#### 验证安装
```bash
rustc --version
cargo --version
```

### 2. 安装FFmpeg

#### Windows
1. 从 https://ffmpeg.org/download.html 下载FFmpeg
2. 解压并将 `bin` 目录添加到系统PATH
3. 验证：`ffmpeg -version`

### 3. 安装项目依赖

```bash
# 进入项目目录
cd bilibili-converter

# 安装Node.js依赖
npm install
```

### 4. 运行开发模式

```bash
# 启动Tauri开发服务器
npm run tauri:dev
```

### 5. 构建生产版本

```bash
# 构建可执行文件
npm run tauri:build
```

构建完成后，可执行文件位于：
- Windows: `src-tauri/target/release/bilibili-converter.exe`

## 项目结构

```
bilibili-converter/
├── src/                      # React前端源码
│   ├── components/           # UI组件
│   │   └── ui/              # shadcn/ui组件
│   ├── lib/                  # 工具函数
│   ├── types/                # TypeScript类型定义
│   ├── App.tsx               # 主应用组件
│   └── main.tsx              # 入口文件
├── src-tauri/                # Rust后端源码
│   ├── src/
│   │   ├── main.rs          # Rust入口
│   │   ├── lib.rs           # 主库文件
│   │   ├── scanner.rs       # 文件扫描模块
│   │   └── converter.rs     # 格式转换模块
│   ├── Cargo.toml           # Rust依赖配置
│   └── tauri.conf.json      # Tauri配置
├── package.json              # Node.js依赖
└── vite.config.ts            # Vite配置
```

## 使用说明

1. 点击"选择文件夹"按钮，选择Bilibili缓存所在目录
2. 程序自动扫描并列出待转换的文件
3. 在格式选择中选择目标格式
4. 点击"开始转换"按钮
5. 界面实时显示转换进度和状态
6. 转换完成后显示结果摘要
7. 可点击文件夹图标打开输出目录

## 注意事项

- 请确保FFmpeg已正确安装并添加到系统PATH
- 转换过程中请勿关闭程序
- 支持的系统：Windows 10/11
