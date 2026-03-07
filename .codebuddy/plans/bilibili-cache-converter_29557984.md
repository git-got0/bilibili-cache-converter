---
name: bilibili-cache-converter
overview: 开发一个Windows桌面应用程序，用于将Bilibili缓存音视频转换为通用格式（MP4/MP3），采用Tauri+React技术栈实现高性能轻量级桌面应用
design:
  architecture:
    framework: react
    component: shadcn
  styleKeywords:
    - Cyberpunk
    - Neon
    - Dark Mode
    - Modern UI
    - Glassmorphism
  fontSystem:
    fontFamily: Microsoft YaHei, PingFang-SC
    heading:
      size: 24px
      weight: 600
    subheading:
      size: 16px
      weight: 500
    body:
      size: 14px
      weight: 400
  colorSystem:
    primary:
      - "#00D9FF"
      - "#8B5CF6"
    background:
      - "#0D1117"
      - "#161B22"
      - "#21262D"
    text:
      - "#FFFFFF"
      - "#8B949E"
    functional:
      - "#10B981"
      - "#EF4444"
      - "#F59E0B"
todos:
  - id: init-tauri-project
    content: Initialize Tauri + React project with scaffolding
    status: completed
  - id: configure-tauri-settings
    content: Configure Tauri settings and window properties
    status: completed
    dependencies:
      - init-tauri-project
  - id: setup-ffmpeg-binary
    content: Download and embed ffmpeg binary
    status: completed
    dependencies:
      - init-tauri-project
  - id: implement-frontend-ui
    content: Build React UI components (folder selector, file list, format menu, progress panel)
    status: completed
    dependencies:
      - init-tauri-project
  - id: implement-rust-scanner
    content: Implement Bilibili cache file scanner in Rust
    status: completed
    dependencies:
      - configure-tauri-settings
  - id: implement-rust-converter
    content: Implement multi-threaded format converter with progress reporting
    status: completed
    dependencies:
      - setup-ffmpeg-binary
  - id: connect-frontend-backend
    content: Connect frontend to Tauri commands and test full workflow
    status: completed
    dependencies:
      - implement-rust-converter
  - id: build-production-exe
    content: Build production executable
    status: completed
    dependencies:
      - connect-frontend-backend
---

## 产品概述

一款面向Windows系统的桌面应用程序，用于将Bilibili缓存的音视频文件转换为通用格式。

## 核心功能

- 文件夹选择：用户通过GUI选择目标文件夹
- 智能扫描：自动识别Bilibili缓存音视频数据（.blv、.m4s等格式）
- 格式转换：支持转换为MP4（视频）、MP3（音频）等通用格式，默认设置MP4/MP3
- 格式选项菜单：提供多种输出格式供用户选择
- 实时监控：界面实时显示运行状态、转换进度和完成情况
- 高并发处理：支持多线程并行转换，提升处理效率

## 用户交互流程

1. 用户点击"选择文件夹"按钮，选择Bilibili缓存所在目录
2. 程序自动扫描并列出待转换的文件
3. 用户在格式选项中选择目标格式（视频：MP4/MKV/AVI，音频：MP3/AAC/FLAC）
4. 用户点击"开始转换"按钮
5. 界面实时显示转换进度和状态
6. 转换完成后显示结果摘要

## 技术栈

- **桌面框架**：Tauri 2.x（Rust后端 + Web前端）
- **前端框架**：React 18 + TypeScript
- **UI组件库**：shadcn/ui + Tailwind CSS
- **音视频处理**：ffmpeg（嵌入到程序中）
- **并发处理**：Rust tokio多线程异步处理

## 技术架构

### 系统架构

- **架构模式**：分层架构（UI层 → 业务逻辑层 → 数据处理层）
- **前端结构**：React组件 → 自定义Hook → Tauri命令调用
- **后端结构**：Tauri命令 → 文件扫描器 → 格式转换器 → 进度管理器

### 模块划分

- **UI组件模块**：文件夹选择器、文件列表、进度条、格式选择菜单、状态显示面板
- **状态管理模块**：React Context用于全局状态管理
- **Tauri命令模块**：scan_folder、convert_file、get_progress、cancel_conversion
- **文件处理模块**：Bilibili缓存识别器（.blv/.m4s检测）、格式转换器、进度追踪器

### 数据流

用户选择文件夹 → Rust后端扫描文件 → 返回文件列表 → 用户选择格式 → 转换任务入队列 → 多线程并发转换 → 进度实时推送前端 → 前端更新UI

## 实现细节

### 核心目录结构

```
d:/workspace-office-automatic/
├── src/                          # React前端源码
│   ├── components/               # UI组件
│   │   ├── FolderSelector.tsx    # 文件夹选择组件
│   │   ├── FileList.tsx          # 文件列表组件
│   │   ├── FormatSelector.tsx    # 格式选择菜单
│   │   ├── ProgressPanel.tsx     # 进度显示面板
│   │   └── StatusBar.tsx         # 状态栏组件
│   ├── hooks/                    # 自定义Hooks
│   │   └── useConverter.ts       # 转换器状态管理
│   ├── types/                    # TypeScript类型定义
│   │   └── index.ts              # 接口和类型定义
│   ├── App.tsx                   # 主应用组件
│   └── main.tsx                  # 入口文件
├── src-tauri/                    # Rust后端源码
│   ├── src/
│   │   ├── main.rs               # Tauri入口
│   │   ├── commands.rs            # Tauri命令定义
│   │   ├── scanner.rs            # 文件扫描模块
│   │   ├── converter.rs          # 格式转换模块
│   │   └── progress.rs           # 进度管理模块
│   ├── Cargo.toml                # Rust依赖配置
│   └── tauri.conf.json           # Tauri配置
├── package.json                  # Node.js依赖
└── SPEC.md                       # 规格说明文档
```

### 性能与可靠性

- **并发策略**：使用Rust tokio创建线程池，默认并发数=CPU核心数
- **进度追踪**：每完成一个文件推送进度事件，避免UI频繁刷新
- **错误处理**：单个文件转换失败不影响其他文件，错误信息记录到日志
- **资源管理**：转换完成后自动释放ffmpeg进程，内存使用可控

## 设计风格

采用现代简约风格，结合Cyberpunk Neon元素，营造科技感与专业感并存的视觉体验。深色主题配合高亮色彩，突出功能区块和操作区域。

## 页面规划

### 主界面（单页面设计）

1. **顶部标题栏**：程序名称和基本信息
2. **文件夹选择区**：选择按钮和路径显示
3. **文件列表区**：待转换文件列表及状态
4. **格式选择区**：视频/音频格式下拉菜单
5. **控制按钮区**：开始转换、取消按钮
6. **进度显示区**：实时进度条和百分比
7. **状态栏**：当前状态文字描述

## 设计规范

### 布局

- 垂直单列布局，从上到下依次排列
- 左侧留白30px，右侧留白30px
- 各区块间距20px
- 卡片圆角12px
- 整体宽度800px，高度600px（可调整）

### 配色

- 主背景：深灰渐变 (#0D1117 → #161B22)
- 卡片背景：半透明深色 (#21262D80)
- 主色调：青色 (#00D9FF)
- 强调色：紫色 (#8B5CF6)
- 成功色：绿色 (#10B981)
- 错误色：红色 (#EF4444)
- 文字色：白色 (#FFFFFF) 和 灰色 (#8B949E)

### 动效

- 按钮悬停：轻微上浮 + 阴影加深
- 进度条：平滑动画过渡
- 卡片：轻微发光边框效果