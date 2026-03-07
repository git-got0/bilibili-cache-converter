# Bilibili缓存转换器 - 技术架构文档

## 目录
- [系统概述](#系统概述)
- [技术栈](#技术栈)
- [架构设计](#架构设计)
- [数据流](#数据流)
- [核心模块](#核心模块)
- [状态管理](#状态管理)
- [并发处理](#并发处理)
- [性能优化](#性能优化)
- [安全性设计](#安全性设计)

---

## 系统概述

### 系统定位
Bilibili缓存转换器是一款基于Tauri框架的高性能桌面应用程序,专门用于将Bilibili缓存的音视频文件转换为通用格式。

### 设计原则
1. **高性能**: 利用Rust的并发能力和FFmpeg的转码效率
2. **用户友好**: 简洁直观的UI,实时进度反馈
3. **可扩展**: 模块化设计,便于功能扩展
4. **跨平台**: 基于Tauri,理论上支持多平台(当前专注于Windows)

### 系统边界
- **输入**: Bilibili缓存文件夹(包含.blv、.m4s等文件)
- **输出**: 通用格式文件(MP4、MKV、AVI、MP3、AAC、FLAC)
- **依赖**: FFmpeg(外部命令行工具)

---

## 技术栈

### 前端技术栈

#### 核心框架
- **React 18.3.1**: UI框架,使用Hooks进行状态管理
- **TypeScript 5.6.2**: 类型安全的JavaScript超集
- **Vite 5.4.10**: 现代化的前端构建工具

#### UI组件库
- **shadcn/ui**: 基于Radix UI的高质量组件库
- **Radix UI**: 无样式、可访问的UI组件原语
- **Tailwind CSS 3.4.17**: 实用优先的CSS框架
- **Lucide React**: 图标库
- **Sonner 2.0.7**: 优雅的Toast通知组件
- **@tanstack/react-virtual 3.13.21**: 高性能虚拟滚动库

#### 开发工具
- **ESLint 9.13.0**: 代码质量检查
- **TypeScript ESLint 8.11.0**: TypeScript专用ESLint规则
- **Prettier 3.4.2**: 代码格式化

### 后端技术栈

#### 核心框架
- **Tauri 2.10.0**: 跨平台桌面应用框架
- **Rust 1.77+**: 系统编程语言,保证性能和安全性

#### 异步运行时
- **Tokio**: Rust异步运行时
- **futures**: 异步编程工具库

#### 并发与同步
- **Arc**: 原子引用计数,实现多线程共享
- **Mutex**: 互斥锁,保护共享状态
- **tokio::sync::Mutex**: 异步互斥锁

#### 配置管理
- **serde**: 序列化/反序列化框架
- **serde_json**: JSON序列化支持

#### 日志记录
- **log**: 日志门面
- **tauri-plugin-log**: Tauri日志插件

### 外部依赖

#### 音视频处理
- **FFmpeg**: 开源音视频处理工具
  - 需要用户手动安装
  - 必须添加到系统PATH
  - 版本要求: 4.x 或更高

---

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                    用户界面 (React)                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │  状态管理 │  │  UI组件   │  │ 事件监听  │ │ 命令调用 │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
└────────────────────────┬────────────────────────────────┘
                         │ Tauri IPC
┌────────────────────────┴────────────────────────────────┐
│                  Tauri Runtime (Rust)                    │
│  ┌──────────────────────────────────────────────────┐  │
│  │           Tauri Command Handler                   │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │  │
│  │  │ scanner │ │converter│ │settings │ │ dialog │ │  │
│  │  └─────────┘ └─────────┘ └─────────┘ └────────┘ │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │              Event Emitter                       │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │           Application State (Arc<Mutex>)         │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────┬────────────────────────────────┘
                         │ System Call
┌────────────────────────┴────────────────────────────────┐
│                     FFmpeg (外部进程)                    │
└─────────────────────────────────────────────────────────┘
```

### 分层架构

#### 表示层 (Presentation Layer)
- **职责**: 用户界面展示和交互
- **技术**: React + TypeScript + Tailwind CSS
- **组件**:
  - 主界面组件 (`App.tsx`)
  - UI基础组件 (`src/components/ui/`)
  - 工具函数 (`src/lib/utils.ts`)

#### 应用层 (Application Layer)
- **职责**: 业务逻辑和状态管理
- **技术**: React Hooks + Tauri Commands
- **功能**:
  - 文件扫描 (`scanner.rs`)
  - 格式转换 (`converter.rs`)
  - 设置管理 (`lib.rs`)

#### 基础设施层 (Infrastructure Layer)
- **职责**: 系统资源管理和外部服务集成
- **技术**: Rust + Tokio
- **组件**:
  - Tauri Runtime
  - FFmpeg集成
  - 文件系统操作

---

## 数据流

### 扫描文件流程

```
用户点击"选择文件夹"
    ↓
selectFolder() 函数
    ↓
调用 dialog.open() 选择文件夹
    ↓
调用 invoke("scan_folder", { folder_path })
    ↓
[后端] scan_folder 命令执行
    ↓
[后端] scanner.rs 递归扫描目录
    ↓
[后端] 识别 Bilibili 缓存文件(.blv, .m4s)
    ↓
[后端] 返回 ScanResult { files, total_size }
    ↓
[前端] 更新 files 和 totalSize 状态
    ↓
[前端] 渲染文件列表
```

### 转换文件流程

```
用户点击"开始转换"
    ↓
startConversion() 函数
    ↓
调用 invoke("start_conversion", { files })
    ↓
[后端] start_conversion 命令执行
    ↓
[后端] 重置 completed_count
    ↓
[后端] 创建异步任务 (tokio::spawn)
    ↓
[后端] converter::convert_files() 并发处理
    ↓
[后端] 对每个文件:
    ├── 更新 current_index
    ├── 调用 convert_single_file()
    │   ├── 启动 FFmpeg 子进程
    │   ├── 监听进度输出
    │   └── 定期发送 conversion-progress 事件
    ├── 增加 completed_count
    └── 保存转换结果
    ↓
[后端] 发送 conversion-complete 事件
    ↓
[前端] 监听 conversion-progress 事件
    ├── 更新进度条
    └── 显示当前文件信息
    ↓
[前端] 监听 conversion-complete 事件
    ├── 显示完成对话框
    ├── 播放提示音
    └── 显示转换结果摘要
```

### 取消转换流程

```
用户点击"取消转换"
    ↓
cancelConversion() 函数
    ↓
调用 invoke("cancel_conversion")
    ↓
[后端] cancel_conversion 命令执行
    ├── 获取当前 completed_count
    ├── 获取 total_count
    ├── 设置 is_converting = false
    ├── 清空 conversion_tasks
    └── 返回 ConversionCancelledEvent
    ↓
[前端] 显示取消对话框
    ├── 显示完成数量
    └── 提供"查看"按钮打开输出文件夹
```

### 暂停/恢复转换流程

```
用户点击"暂停转换"
    ↓
pauseConversion() 函数
    ↓
调用 invoke("pause_conversion")
    ↓
[后端] pause_conversion 命令执行
    ├── 检查 is_converting 状态
    ├── 检查 is_paused 状态
    ├── 设置 is_paused = true
    ├── 获取 completed_count 和 pending_count
    ├── 返回 ConversionPausedEvent
    └── 发送 conversion-paused 事件
    ↓
[前端] 更新 isPaused 状态
    ├── 更新UI显示"暂停中"
    └── 切换按钮为"恢复转换"

用户点击"恢复转换"
    ↓
resumeConversion() 函数
    ↓
调用 invoke("resume_conversion")
    ↓
[后端] resume_conversion 命令执行
    ├── 检查 is_converting 状态
    ├── 检查 is_paused 状态
    ├── 设置 is_paused = false
    ├── 获取 completed_count 和 pending_count
    ├── 返回 ConversionResumedEvent
    └── 发送 conversion-resumed 事件
    ↓
[前端] 更新 isPaused 状态
    ├── 更新UI显示"转换中"
    └── 切换按钮为"暂停转换"
```

**实现细节**:
- **暂停机制**: 通过 `is_paused` 状态标志控制转换任务的执行
- **恢复机制**: 清除暂停标志，转换任务继续执行
- **状态管理**: `AppState` 包含 `is_paused: Mutex<bool>`
- **事件通知**: 前端通过事件监听更新UI状态

---

## 核心模块

### 1. 前端主应用模块 (src/App.tsx)

#### 职责
- 界面渲染
- 状态管理
- 事件处理
- 用户交互

#### 关键状态
```typescript
interface AppState {
  folderPath: string;              // 输入文件夹路径
  outputPath: string;              // 输出文件夹路径
  defaultOutputPath: string;       // 默认输出路径
  files: MediaFile[];              // 待转换文件列表
  isScanning: boolean;             // 是否正在扫描
  isConverting: boolean;           // 是否正在转换
  isPaused: boolean;              // 是否已暂停
  progress: ConversionProgress;    // 转换进度
  settings: AppSettings;            // 应用设置
  // ... 对话框状态
}
```

#### 关键函数
- `selectFolder()`: 选择输入文件夹
- `selectOutputFolder()`: 选择输出文件夹
- `startConversion()`: 开始转换
- `cancelConversion()`: 取消转换
- `openOutputFolder()`: 打开输出文件夹
- `openDefaultOutputFolder()`: 打开默认输出文件夹

### 2. 文件扫描模块 (src-tauri/src/scanner.rs)

#### 职责
- 递归扫描指定目录
- 识别Bilibili缓存文件
- 提取文件元数据
- 计算文件总大小

#### 核心函数
```rust
pub async fn scan_bilibili_files(
    folder_path: &str,
    app: Option<AppHandle>
) -> Result<ScanResult>
```

#### 识别规则
- 文件扩展名: `.blv`, `.m4s`, `.mp4`, `.flv`
- 文件路径匹配: Bilibili缓存目录结构
- 文件类型识别: 视频/音频

#### 实现细节
- 使用 `tokio::fs` 异步文件系统操作
- 使用 `walkdir` 遍历目录树
- 使用 `regex` 匹配文件路径

### 3. 格式转换模块 (src-tauri/src/converter.rs)

#### 职责
- 调用FFmpeg进行格式转换
- 监控转换进度
- 管理并发转换任务
- 处理转换结果

#### 核心结构
```rust
pub struct ConversionTask {
    id: String,
    file: MediaFile,
    settings: AppSettings,
    handle: Option<JoinHandle<Result<ConversionResult>>>,
}
```

#### 核心函数
```rust
pub async fn convert_files(
    app: AppHandle,
    files: Vec<MediaFile>,
    settings: &AppSettings,
    state: Arc<AppState>
) -> Vec<ConversionResult>

async fn convert_single_file(
    app: AppHandle,
    file: &MediaFile,
    settings: &AppSettings,
    current_index: usize,
    total_count: usize
) -> Result<ConversionResult>
```

#### FFmpeg命令构建
```rust
// 视频转换示例
let args = vec![
    "-i", &input_path,
    "-c:v", "libx264",
    "-c:a", "aac",
    "-movflags", "+faststart",
    &output_path
];

// 音频转换示例
let args = vec![
    "-i", &input_path,
    "-vn",
    "-acodec", "libmp3lame",
    "-ab", "192k",
    &output_path
];
```

#### 进度监控
- 解析FFmpeg的stderr输出
- 匹配时间信息 `time=HH:MM:SS.ms`
- 计算进度百分比
- **性能指标追踪**:
  - `conversion_speed`: 实时转换速度(MB/s)
  - `average_speed`: 平均转换速度(MB/s)
  - `estimated_size`: 预计输出大小(字节)
  - `processed_bytes`: 已处理字节数
  - `elapsed_time`: 已用时间(秒)
  - `remaining_time`: 预计剩余时间(秒)

### 4. 状态管理模块 (src-tauri/src/lib.rs)

#### 职责
- 管理应用全局状态
- 注册Tauri命令
- 处理事件发射

#### 核心结构
```rust
pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub conversion_tasks: Mutex<HashMap<String, ConversionTask>>,
    pub is_converting: Mutex<bool>,
    pub completed_count: Mutex<usize>,
}
```

#### Tauri命令注册
```rust
.invoke_handler(tauri::generate_handler![
    scan_folder,
    start_conversion,
    cancel_conversion,
    pause_conversion,
    resume_conversion,
    get_settings,
    update_settings,
    open_output_folder,
    ensure_output_directory,
    get_ffmpeg_path,
    get_default_output_path,
])
```

---

## 状态管理

### 前端状态管理

#### 状态类型
1. **UI状态**: 对话框显示/隐藏
2. **数据状态**: 文件列表、设置
3. **过程状态**: 扫描中、转换中

#### 状态更新模式
```typescript
// 使用 useState Hook
const [files, setFiles] = useState<MediaFile[]>([]);

// 使用 useEffect 监听事件
useEffect(() => {
  const unlisten = listen<ConversionProgress>("conversion-progress", (event) => {
    setProgress(event.payload);
  });
  return () => unlisten.then(fn => fn());
}, []);

// 监听暂停/恢复事件
useEffect(() => {
  const unlistenPaused = listen("conversion-paused", () => {
    setIsPaused(true);
  });
  const unlistenResumed = listen("conversion-resumed", () => {
    setIsPaused(false);
  });
  return () => {
    unlistenPaused.then(fn => fn?.());
    unlistenResumed.then(fn => fn?.());
  };
}, []);
```

### 后端状态管理

#### 状态容器
```rust
pub struct AppState {
    pub settings: Mutex<AppSettings>,           // 应用设置
    pub conversion_tasks: Mutex<HashMap<String, ConversionTask>>,  // 转换任务
    pub is_converting: Mutex<bool>,             // 转换标志
    pub is_paused: Mutex<bool>,                 // 暂停标志
    pub completed_count: Mutex<usize>,           // 完成计数
}
```

#### 状态访问模式
```rust
// 读取状态
let settings = state.settings.lock().await.clone();

// 更新状态
let mut settings = state.settings.lock().await;
*settings = new_settings;

// 原子操作
{
    let mut is_converting = state.is_converting.lock().await;
    *is_converting = true;
}
```

#### 状态共享
- 使用 `Arc<AppState>` 在不同任务间共享状态
- 使用 `Mutex` 保证线程安全
- 使用 `async Mutex` 支持异步环境

---

## 并发处理

### 并发模型

#### 异步任务池
```rust
// 创建并发任务
let handles: Vec<_> = files.chunks(concurrency)
    .enumerate()
    .map(|(chunk_index, chunk)| {
        let app_clone = app.clone();
        let state_arc = state.clone();
        let chunk = chunk.to_vec();
        let settings = settings.clone();

        tokio::spawn(async move {
            for (index, file) in chunk.iter().enumerate() {
                let global_index = chunk_index * concurrency + index;
                convert_single_file(
                    app_clone.clone(),
                    file,
                    &settings,
                    global_index,
                    total_files
                ).await?;
            }
            Ok(())
        })
    })
    .collect();
```

### 并发控制

#### 并发数配置
- 默认值: CPU核心数
- 可选值: 1, 2, 4, 8
- 用户可在设置中调整

#### 资源限制
- 文件句柄限制
- 内存使用限制
- CPU使用率控制

### 线程安全

#### 共享状态保护
```rust
// 使用 Mutex 保护共享状态
pub conversion_tasks: Mutex<HashMap<String, ConversionTask>>;

// 使用 Arc 实现跨线程共享
let state_arc = Arc::new(AppState::default());
```

#### 死锁预防
- 避免嵌套锁
- 及时释放锁
- 使用 `drop()` 显式释放

---

## 性能优化

### 1. 前端优化

#### 渲染优化
```typescript
// 使用 React.memo 避免不必要的重渲染
const FileItem = React.memo(({ file }: { file: MediaFile }) => {
  return <div>{file.name}</div>;
});

// 使用 key 属性优化列表渲染
{files.map(file => (
  <FileItem key={file.id} file={file} />
))}
```

#### 事件节流
```typescript
// 使用自定义节流Hook限制更新频率
const progressThrottleRef = useRef(createThrottledState<ConversionProgress | null>(null, 150));

// 监听进度事件
const unlistenProgress = listen<ConversionProgress>("conversion-progress", (event) => {
  const throttled = progressThrottleRef.current;
  const applied = throttled.setValue(event.payload);
  if (applied !== null) {
    setProgress(applied);
  }
});

// 定期强制刷新(200ms间隔)
const flushInterval = setInterval(() => {
  const throttled = progressThrottleRef.current;
  const value = throttled.forceFlush();
  if (value !== null) {
    setProgress(value);
  }
}, 200);
```

#### 虚拟滚动
- **实现位置**: `src/hooks/useVirtualList.ts`
- **库**: `@tanstack/react-virtual`
- **触发条件**: 文件数超过100时自动启用
- **优化效果**:
  - 仅渲染可视区域内的文件项
  - 大幅减少DOM节点数量
  - 提升大量文件时的滚动性能
- **实现细节**:
  ```typescript
  const virtualList = useVirtualList({
    items: files,
    itemHeight: 36,         // 单项高度(像素)
    containerHeight: 150,   // 容器高度(像素)
    overscan: 5,            // 溢出缓冲项数
  });
  ```

### 2. 后端优化

#### 异步I/O
```rust
// 使用 tokio::fs 替代 std::fs
use tokio::fs;

let metadata = fs::metadata(&path).await?;
let content = fs::read(&path).await?;
```

#### 内存复用
- 重用缓冲区
- 避免频繁的内存分配

#### 批量操作
- 批量读取文件元数据
- 批量更新进度

### 3. FFmpeg优化

#### 命令参数优化
```rust
// 硬件加速(如果可用)
let args = vec![
    "-hwaccel", "auto",
    "-i", &input_path,
    "-c:v", "h264_nvenc",  // NVIDIA NVENC
    // ...
];
```

#### 编码参数调整
```rust
// 平衡质量和速度
let args = vec![
    "-crf", "23",              // 质量因子
    "-preset", "medium",       // 编码速度预设
    "-threads", "4",           // 线程数
];
```

---

## 安全性设计

### 1. 输入验证与路径防护 (v1.0.2+)

#### 路径验证
```rust
use std::path::Path;

fn validate_path(path: &str) -> Result<String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err("路径不存在".to_string());
    }
    if !path.is_dir() {
        return Err("不是有效的目录".to_string());
    }
    Ok(path.to_string_lossy().to_string())
}
```

#### 文件名清理 (v1.0.2+)
```rust
fn sanitize_filename(name: &str) -> String {
    // 防止路径遍历攻击
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0', '\n', '\r', '\t'];
    let mut result = String::new();

    for c in name.chars() {
        if invalid_chars.contains(&c) {
            result.push('_');
        } else {
            // 替换连续的点号，防止路径遍历
            if c == '.' {
                if result.ends_with('.') {
                    result.push('_');
                } else {
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        }
    }

    let trimmed = result.trim();
    if trimmed.is_empty() {
        "output".to_string()
    } else {
        trimmed.to_string()
    }
}
```

### 2. 路径遍历防护 (v1.0.2+)

#### 输出路径验证
```rust
// 验证输出路径在预期目录内
let output_path_obj = Path::new(&output_path_str);
let output_dir_obj = Path::new(output_dir);

match output_path_obj.canonicalize() {
    Ok(canonical_output) => {
        let canonical_dir = output_dir_obj.canonicalize()
            .unwrap_or_else(|_| output_dir_obj.to_path_buf());
        if !canonical_output.starts_with(&canonical_dir) {
            return Err("路径遍历检测:输出路径超出预期目录".to_string());
        }
    }
    Err(_) => { /* 新文件，检查父目录 */ }
}
```

#### 路径参数验证
```rust
// open_output_folder 命令
async fn open_output_folder(folder_path: String) -> Result<(), String> {
    let path = Path::new(&folder_path);
    
    // 验证绝对路径
    if !path.is_absolute() {
        return Err("无效路径:必须是绝对路径".to_string());
    }
    
    // 验证存在且为目录
    if !path.exists() || !path.is_dir() {
        return Err("目录不存在".to_string());
    }
    
    // 执行操作...
    Ok(())
}
```

#### 类型验证
```typescript
// TypeScript 编译时类型检查
interface MediaFile {
  id: string;
  path: string;
  size: number;
  // ...
}
```

### 3. 权限控制

#### Tauri权限配置
```json
// src-tauri/capabilities/default.json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default",
    "fs:allow-read-file",
    "fs:allow-write-file",
    "fs:allow-read-dir",
    "fs:allow-write-dir",
    "shell:allow-open",
    "notification:default"
  ]
}
```

#### 文件系统访问
- 仅允许访问用户指定的目录
- 禁止访问系统敏感目录

### 3. 错误处理

#### 错误传播
```rust
pub async fn scan_folder(
    folder_path: String
) -> Result<ScanResult, String> {
    let path = validate_path(&folder_path)?;
    let files = scan_files(&path).await?;
    Ok(ScanResult { files, total_size })
}
```

#### 错误日志
```rust
log::error!("扫描失败: {}", error);
```

### 5. 资源限制

#### 进程管理
```rust
// 限制 FFmpeg 子进程数
let max_processes = settings.concurrency;
```

#### 内存管理
```rust
// 使用 Box 减少栈内存使用
let data: Box<[u8]> = Box::new([0; 1024 * 1024]);
```

---

## 扩展性设计

### 1. 插件架构

#### 转换器插件接口
```rust
pub trait ConverterPlugin {
    async fn convert(
        &self,
        input: &str,
        output: &str,
        settings: &ConversionSettings
    ) -> Result<ConversionResult>;
}
```

### 2. 配置驱动

#### 格式配置
```json
{
  "formats": {
    "video": {
      "mp4": {
        "encoder": "libx264",
        "extension": "mp4",
        "default_args": ["-crf", "23", "-preset", "medium"]
      }
    }
  }
}
```

### 3. 事件驱动

#### 自定义事件
```rust
// 定义新事件
#[derive(Serialize, Deserialize)]
pub struct CustomEvent {
    event_type: String,
    data: serde_json::Value,
}

// 发送事件
app.emit("custom-event", event)?;
```

---

## 版本更新日志

### v1.0.1 (2026-03-06)

#### 核心优化

**1. 智能文件命名系统**
- **实现位置**: `src-tauri/src/scanner.rs`
- **功能描述**:
  - 从 `entry.json` 中提取 `part` 和 `title` 字段
  - 优先级: `part` → `title` → 回退到目录标题
  - 自动截断过长的文件名（限制50字符）
  - 支持多个路径查找: `part`, `page_data.part`, `video_info.part`, `data.part`

**2. 输出路径优化**
- **实现位置**: `src-tauri/src/converter.rs`, `src-tauri/src/lib.rs`
- **修复内容**:
  - 从前端传入正确的用户选择路径作为 `base_dir`
  - result 目录始终位于用户选择的根目录下
  - 保持原始相对路径结构
  - 移除了错误的硬编码 `.ancestors().nth(3)` 逻辑

**3. 音频处理优化**
- **实现位置**: `src-tauri/src/scanner.rs`
- **功能描述**:
  - 扫描阶段跳过与视频配套的音频文件
  - 结果过滤时只保留视频文件
  - 音频文件会在转换时合并到视频中

#### 代码质量改进

**1. 修复 Clippy 警告**
- 合并嵌套的 if 语句
- 移除不必要的 `&` 引用
- 删除未使用的 `generate_output_name` 函数

**2. JSON 解析修复**
- 使用 `pointer()` 方法正确读取嵌套路径
- 路径格式从 `page_data.title` 改为 `page_data/title` (JSON Pointer 规范)

**3. 构建脚本优化**
- 更新 `build.bat` 使用正确的 Tauri 构建命令
- 分步构建: 前端 → 后端 → 打包
- 显示构建输出位置

#### 技术细节

**文件命名逻辑** (`generate_output_name_with_part`):
```rust
// 1. 优先使用 part 字段
if let Some(part) = extract_part_from_json(&content) {
    return format!("{}_P{}.{}", safe_title, part, ext);
}

// 2. 回退到 title 字段
if let Some(json_title) = extract_title_from_json(&content) {
    let safe_json_title = sanitize_filename(&json_title);
    let truncated_title = truncate_chinese(&safe_json_title, 50);
    return format!("{}.{}", truncated_title, ext);
}

// 3. 最终回退
format!("{}.{}", truncate_chinese(&safe_title, 50), ext)
```

**输出路径计算**:
```rust
// 修复前: 错误地硬编码上溯3级
let base_dir = path.ancestors().nth(3).unwrap_or(...);

// 修复后: 使用前端传入的正确路径
pub async fn start_conversion(
    app: AppHandle,
    files: Vec<MediaFile>,
    folder_path: String,  // 新增参数
    state: State<'_, Arc<AppState>>,
) -> Result<(), String>
```

#### 构建产物
- 可执行文件: `src-tauri/target/release/bilibili-converter.exe` (7.0 MB)
- 安装程序: `src-tauri/target/release/bundle/nsis/Bilibili缓存转换器_1.0.0_x64-setup.exe` (67.9 MB)

---

**文档版本**: 1.0.0
**最后更新**: 2026年3月7日
**架构师**: [待补充]
