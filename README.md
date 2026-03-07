# Bilibili缓存转换器

一款高性能的桌面应用程序,用于将Bilibili缓存的音视频文件转换为通用格式(如MP4、MP3等)。

## 项目信息

- **版本**: 1.0.0
- **最后更新**: 2026年3月7日
- **开发语言**: Rust + TypeScript + React
- **构建工具**: Tauri + Vite
- **目标平台**: Windows 10/11 (64位)

## 核心功能

### 1. 文件扫描与识别
- 自动扫描Bilibili缓存文件夹
- 智能识别视频文件(.blv、.m4s、.flv、.ts)和音频文件(.aac)
- 提取文件元数据和标题信息
- 计算文件总大小

### 2. 格式转换
- 支持视频格式: MP4、MKV、AVI
- 支持音频格式: MP3、AAC、FLAC
- 基于FFmpeg的高性能转码引擎
- 支持硬件加速(NVIDIA NVENC、AMD AMF、Intel QSV)
- 智能合并video.m4s和audio.m4s

### 3. 智能文件命名
- 从entry.json提取part和title字段
- 优先使用part字段(例: 视频标题_P1.mp4)
- 自动截断过长的文件名(限制50字符)

### 4. 智能目录结构优化
- 保留原始相对路径结构
- 自动精简单层文件夹(当上级文件夹只有一个子文件夹时)
- 确保至少保留次顶级目录(指定目录的子目录层)

### 5. 实时进度监控
- 动态显示整体任务进度
- 实时显示单个文件转换进度
- 性能指标: 转换速度、平均速度、预计剩余时间、输出大小
- 支持中途取消任务

### 6. 任务控制
- 取消功能: 运行中途可取消任务
- 暂停功能: 支持暂停正在进行的转换任务
- 恢复功能: 从暂停状态恢复转换
- 查看功能: 直接打开输出文件夹

### 7. 性能优化
- 虚拟滚动: 大量文件时自动启用虚拟滚动
- 事件节流: 进度更新使用节流机制(150ms间隔)
- 并发控制: 支持自定义并发数(1/2/4/8)

## 快速开始

### 环境要求

#### 开发环境
- Node.js 18+
- npm 9+
- Rust 1.77+
- FFmpeg 4.x+

#### 生产环境
- Windows 10/11 (64位)
- FFmpeg 4.x+ (必须添加到系统PATH)

### 安装步骤

1. **安装Node.js依赖**
```bash
npm install
```

2. **安装Rust** (如未安装)
```powershell
# Windows
rustup-init.exe -y
```

3. **安装FFmpeg**
```powershell
# 使用Chocolatey
choco install ffmpeg -y

# 或手动安装并添加到系统PATH
```

4. **运行开发环境**
```bash
npm run tauri:dev
```

### 构建生产版本

```bash
# 构建完整应用
npm run tauri:build

# 构建产物位置
# 可执行文件: src-tauri/target/release/bilibili-converter.exe
# 安装包: src-tauri/target/release/bundle/nsis/Bilibili缓存转换器_1.0.0_x64-setup.exe
```

## 项目结构

```
bilibili-converter/
├── src/                    # 前端源码
│   ├── components/         # UI组件
│   │   └── ui/            # shadcn/ui基础组件
│   ├── hooks/              # 自定义Hooks
│   ├── lib/                # 工具函数
│   ├── types/              # 类型定义
│   ├── App.tsx             # 主应用组件
│   └── main.tsx            # 应用入口
├── src-tauri/              # 后端源码
│   ├── src/
│   │   ├── lib.rs          # 主库文件
│   │   ├── converter.rs    # 格式转换模块
│   │   └── scanner.rs      # 文件扫描模块
│   ├── Cargo.toml          # Rust依赖配置
│   └── tauri.conf.json     # Tauri配置
├── public/                 # 静态资源
├── dist/                   # 前端构建输出
├── API_DOCUMENTATION.md     # API接口文档
├── DEPLOYMENT_GUIDE.md     # 部署指南
├── PROJECT_README.md       # 项目归档文档
├── TECHNICAL_ARCHITECTURE.md # 技术架构文档
└── package.json            # Node.js依赖配置
```

## 技术栈

### 前端
- **框架**: React 18.3.1
- **语言**: TypeScript 5.6.2
- **构建工具**: Vite 5.4.10
- **UI组件**: shadcn/ui (Radix UI + Tailwind CSS)
- **状态管理**: React Hooks
- **虚拟滚动**: @tanstack/react-virtual
- **通知组件**: Sonner

### 后端
- **框架**: Tauri 2.10.0
- **语言**: Rust 1.77+
- **异步运行时**: Tokio
- **音视频处理**: FFmpeg
- **并发处理**: tokio::spawn + Arc<Mutex>

## 文档

- [API接口文档](./API_DOCUMENTATION.md) - 前后端接口详细说明
- [技术架构文档](./TECHNICAL_ARCHITECTURE.md) - 系统架构和技术细节
- [部署指南](./DEPLOYMENT_GUIDE.md) - 部署和维护手册
- [项目归档文档](./PROJECT_README.md) - 完整的项目文档

## 常见问题

### 1. FFmpeg未找到
确保FFmpeg已安装并添加到系统PATH:
```powershell
ffmpeg -version
```

### 2. 转换失败
检查:
- FFmpeg版本是否兼容
- 源文件是否损坏
- 输出路径是否有写入权限

### 3. 性能问题
降低并发数设置,避免系统资源过度占用。

## 许可证

MIT License

## 贡献

欢迎提交Issue和Pull Request!

## 联系方式

如有问题或建议,请通过以下方式联系:
- 项目仓库: [待补充]
- 邮箱: [待补充]
