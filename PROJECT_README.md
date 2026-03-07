# Bilibili缓存转换器 - 项目归档文档

## 项目概述

一款面向Windows系统的高性能桌面应用程序,用于将Bilibili缓存的音视频文件转换为通用格式(如MP4、MP3等)。

### 项目版本
- **版本号**: 1.0.0
- **最后更新**: 2026年3月7日
- **构建产物**:
  - 可执行文件: `src-tauri/target/release/bilibili-converter.exe`
  - 安装包: `src-tauri/target/release/bundle/nsis/Bilibili缓存转换器_1.0.0_x64-setup.exe`

## 核心功能

### 1. 文件夹选择与管理
- 支持GUI选择Bilibili缓存目录
- 自动扫描并识别音视频缓存文件(.blv、.m4s等)
- 支持自定义输出文件夹或使用默认路径

### 2. 智能格式转换
- 支持视频格式: MP4、MKV、AVI
- 支持音频格式: MP3、AAC、FLAC
- 基于FFmpeg的高性能转码引擎
- **智能文件命名**:
  - 从 entry.json 中读取 part 和 title 字段
  - 有 part 时: `{title}_P{part}.{ext}` (例: `视频标题_P1.mp4`)
  - 无 part 时: 使用 json 中的 title 字段作为文件名
  - 自动截断过长的文件名（限制50字符）
- **音频合并优化**:
  - 同一目录下的 video.m4s 和 audio.m4s 自动合并输出
  - 已被合并的音频文件不再单独转换
- **智能目录结构优化**:
  - 保留原始相对路径结构
  - 自动精简单层文件夹(当上级文件夹只有一个子文件夹时)
  - 确保至少保留次顶级目录(指定目录的子目录层)

### 3. 实时进度监控
- 动态显示整体任务进度(当前文件/总文件数)
- 实时显示单个文件转换进度
- 支持中途取消任务并显示完成情况
- **性能指标显示**:
  - 实时转换速度 (MB/s)
  - 平均转换速度
  - 预计剩余时间
  - 输出文件大小预估
  - 已处理字节数

### 4. 任务控制
- **取消功能**: 运行中途可取消任务,弹出进度提示框
- **暂停功能**: 支持暂停正在进行的转换任务
- **恢复功能**: 从暂停状态恢复转换
- **查看功能**: 支持直接打开输出文件夹
- **默认路径**: 未设置输出文件夹时,可打开默认输出路径(result文件夹)
- **正确的输出路径**:
  - result 目录始终位于用户选择的根目录下
  - 示例: 选择 `download` 文件夹，文件位于 `download/v/video.blv`
  - 输出位置: `download/result/v/video.mp4`
  - 保持原始相对路径结构
- **设置管理**: 支持提示音开关、并发数配置、输出路径设置

### 5. 高性能渲染
- **虚拟滚动**: 大量文件(>100)时自动启用虚拟滚动
  - 仅渲染可视区域内的文件
  - 大幅减少DOM节点数量
  - 提升滚动性能和响应速度
- **事件节流**: 进度更新使用节流机制(150ms间隔)
  - 减少UI重渲染频率
  - 优化CPU使用率

### 6. 响应式设计
- 支持窗口大小自由调整(最小窗口: 600x500)
- 自适应不同分辨率屏幕
- 优化文件列表滚动,防止内容被挤出可视区域

### 6. 系统集成
- 系统托盘支持
- 任务完成通知
- 右键菜单(显示窗口、退出)

## 技术架构

### 前端技术栈
- **框架**: React 18.3.1
- **语言**: TypeScript 5.6.2
- **构建工具**: Vite 5.4.10
- **UI组件**: shadcn/ui (Radix UI + Tailwind CSS)
- **虚拟滚动**: @tanstack/react-virtual 3.13.21
- **状态管理**: React Hooks (useState, useEffect)
- **通知组件**: Sonner

### 后端技术栈
- **框架**: Tauri 2.10.0
- **语言**: Rust 1.77+
- **异步运行时**: Tokio
- **音视频处理**: FFmpeg (外部依赖)
- **并发处理**: tokio::spawn + Arc<Mutex>

### 核心模块

#### 前端模块 (`src/`)
```
src/
├── components/
│   └── ui/              # shadcn/ui基础组件
│       ├── button.tsx   # 按钮组件
│       ├── dialog.tsx   # 对话框组件
│       ├── label.tsx    # 标签组件
│       ├── progress.tsx # 进度条组件
│       ├── select.tsx   # 下拉选择组件
│       └── switch.tsx   # 开关组件
├── hooks/
│   ├── useThrottle.ts   # 节流Hook(用于性能优化)
│   └── useVirtualList.ts # 虚拟滚动Hook
├── lib/
│   └── utils.ts         # 工具函数(cn、formatFileSize)
├── types/
│   └── index.ts         # TypeScript类型定义
├── App.tsx              # 主应用组件
└── main.tsx             # 应用入口
```

#### 后端模块 (`src-tauri/src/`)
```
src-tauri/src/
├── lib.rs               # 主库文件(状态管理、Tauri命令)
├── converter.rs         # 格式转换模块
└── scanner.rs           # 文件扫描模块
```

### 数据流架构

```
用户操作 → React组件 → Tauri Invoke → Rust命令处理
                                    ↓
                            事件发射器(Tauri Events)
                                    ↓
                            React事件监听 → UI更新
```

## API接口文档

### Tauri Commands (前端 → 后端)

#### 1. scan_folder
扫描指定文件夹,识别Bilibili缓存文件

**参数**:
- `folder_path: string` - 要扫描的文件夹路径

**返回**:
```typescript
{
  files: Array<{
    id: string;
    path: string;
    name: string;
    size: number;
    file_type: "video" | "audio";
    title: string;
    output_name: string;
    has_audio?: boolean;
  }>;
  total_size: number;
}
```

#### 2. start_conversion
启动批量转换任务

**参数**:
- `files: MediaFile[]` - 待转换的文件列表

**返回**: `void`

**触发事件**: `conversion-progress`, `conversion-complete`

#### 3. cancel_conversion
取消正在进行的转换任务

**参数**: 无

**返回**:
```typescript
{
  completed_count: number;
  total_count: number;
}
```

**触发事件**: `conversion-cancelled`

#### 4. get_settings
获取应用设置

**返回**:
```typescript
{
  sound_enabled: boolean;
  output_format_video: string;
  output_format_audio: string;
  output_path: string;
  concurrency: number;
}
```

#### 5. update_settings
更新应用设置

**参数**: `newSettings: AppSettings`

**返回**: `void`

#### 6. open_output_folder
打开指定文件夹

**参数**:
- `folder_path: string` - 要打开的文件夹路径

**返回**: `void`

#### 7. ensure_output_directory
确保输出目录存在,不存在则创建

**参数**:
- `path: string` - 目录路径

**返回**: `void`

#### 8. get_default_output_path
根据源文件夹路径计算默认输出路径

**参数**:
- `folder_path: string` - 源文件夹路径

**返回**: `string` - 默认输出路径(源文件夹/result)

### Tauri Events (后端 → 前端)

#### 1. conversion-progress
转换进度更新事件

**事件数据**:
```typescript
{
  file_id: string;
  file_name: string;
  progress: number;        // 0-100, 单个文件进度
  status: string;
  current_index: number;   // 当前文件索引
  total_count: number;    // 总文件数
}
```

**整体进度计算**: `(current_index * 100 + progress) / total_count`

#### 2. conversion-complete
转换完成事件

**事件数据**:
```typescript
{
  success_count: number;
  total_count: number;
  results: Array<{
    file_id: string;
    success: boolean;
    output_path?: string;
    error?: string;
  }>;
}
```

#### 3. conversion-cancelled
转换取消事件

**事件数据**:
```typescript
{
  completed_count: number;
  total_count: number;
}
```

#### 4. play-notification-sound
播放通知声音事件

#### 5. scan-progress
扫描进度事件(可选)

**事件数据**:
```typescript
{
  found_files: number;
  current_path: string;
}
```

## 部署与维护

### 环境依赖

#### 开发环境
- Node.js 18+
- npm 9+
- Rust 1.77+
- Cargo

#### 运行时依赖
- FFmpeg (必须添加到系统PATH)
- Windows 10/11 (64位)

### 构建流程

#### 1. 开发环境设置
```bash
# 安装Node.js依赖
npm install

# 安装Rust工具链(如未安装)
rustup-init.exe -y

# 验证环境
node -v
npm -v
rustc --version
cargo --version
ffmpeg -version
```

#### 2. 开发模式运行
```bash
npm run tauri:dev
```

#### 3. 生产构建
```bash
# 前端构建
npm run build

# 完整应用构建(包含前端和后端)
npm run tauri:build
```

#### 4. 构建产物
```
src-tauri/
├── target/
│   └── release/
│       ├── bilibili-converter.exe                    # 可执行文件
│       └── bundle/
│           └── nsis/
│               └── Bilibili缓存转换器_1.0.0_x64-setup.exe  # 安装包
```

### 发布流程

1. **版本更新**: 修改 `package.json` 和 `src-tauri/Cargo.toml` 中的版本号
2. **测试验证**: 在开发环境充分测试所有功能
3. **构建发布**: 执行 `npm run tauri:build`
4. **分发**: 发布安装包至目标平台

### 维护建议

#### 1. 定期更新依赖
```bash
# 检查过时的依赖
npm outdated

# 更新依赖
npm update

# 更新Tauri CLI
cargo install tauri-cli --version "^2.0.0"
```

#### 2. 性能监控
- 监控转换耗时
- 优化并发参数
- 定期清理缓存文件

#### 3. 用户反馈收集
- 记录常见问题
- 收集功能改进建议
- 持续优化用户体验

## 开发交接指南

### 快速上手

1. **环境准备**: 安装Node.js、Rust、FFmpeg
2. **项目克隆**: 克隆项目仓库
3. **依赖安装**: `npm install`
4. **开发运行**: `npm run tauri:dev`
5. **代码阅读**: 按照"核心模块"结构阅读代码

### 关键代码位置

| 功能模块 | 文件位置 | 说明 |
|---------|---------|------|
| 主界面 | `src/App.tsx` | 所有UI组件和交互逻辑 |
| 类型定义 | `src/types/index.ts` | 前后端共享类型 |
| 文件扫描 | `src-tauri/src/scanner.rs` | Bilibili缓存识别逻辑 |
| 格式转换 | `src-tauri/src/converter.rs` | FFmpeg调用和并发处理 |
| 状态管理 | `src-tauri/src/lib.rs` | AppState、Tauri命令 |
| UI组件 | `src/components/ui/` | shadcn/ui基础组件 |

### 扩展开发指南

#### 添加新的输出格式
1. 修改 `src/types/index.ts` 添加格式选项
2. 在 `src/App.tsx` 的格式选择下拉框中添加选项
3. 更新 `src-tauri/src/converter.rs` 的FFmpeg命令参数

#### 添加新功能模块
1. 在 `src-tauri/src/` 创建新模块文件
2. 在 `lib.rs` 中引入模块
3. 使用 `#[tauri::command]` 暴露命令
4. 在 `run()` 函数的 `invoke_handler!` 中注册命令
5. 前端使用 `invoke()` 调用命令

#### UI组件定制
1. 基于 shadcn/ui 组件进行扩展
2. 使用 Tailwind CSS 进行样式定制
3. 保持响应式设计,使用 `flex-wrap`、`grid-cols` 等响应式类

### 常见问题排查

#### 问题1: FFmpeg未找到
**症状**: 转换失败,日志显示 "FFmpeg not found"
**解决**:
- 确保FFmpeg已安装
- 将FFmpeg的bin目录添加到系统PATH
- 重启应用程序

#### 问题2: 转换卡死
**症状**: 进度条不动,程序无响应
**解决**:
- 检查并发数设置,降低并发数
- 检查源文件是否损坏
- 查看控制台日志获取错误信息

#### 问题3: 窗口显示异常
**症状**: 界面元素错位或被截断
**解决**:
- 调整窗口大小
- 检查 `tauri.conf.json` 中的最小窗口尺寸设置
- 确认屏幕分辨率支持

## 项目清理建议

### 可清理的文件/目录
- `dist/` - 前端构建输出(可重新生成)
- `src-tauri/target/debug/` - Debug编译产物
- `src-tauri/target/release/` - Release编译产物(可重新生成)
- `.vscode/` - VSCode配置(如不需要)
- `*.log` - 日志文件

### 应保留的文件/目录
- `src/` - 前端源码
- `src-tauri/src/` - 后端源码
- `package.json` - 依赖配置
- `src-tauri/Cargo.toml` - Rust依赖配置
- `src-tauri/tauri.conf.json` - Tauri配置
- 所有配置文件(tsconfig, vite.config, tailwind.config等)

## 许可与版权

本项目为自研项目,用于个人学习和使用。

## 联系与支持

如有问题或建议,请通过以下方式联系:
- 项目仓库: [待补充]
- 邮箱: [待补充]

---

**文档版本**: 1.0.0
**最后更新**: 2026年3月7日
**维护者**: [待补充]
