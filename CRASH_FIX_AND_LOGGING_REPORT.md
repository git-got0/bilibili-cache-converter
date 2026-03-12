# 崩溃修复和日志改进报告

## 概述

本次更新旨在修复可能导致程序闪退的代码问题，并改进日志记录功能，确保日志能够成功生成并保存到本地。

## 修复的主要问题

### 1. 防止 Panic 导致程序崩溃

#### 问题 1.1: 任务执行中的 Panic
**位置**: `src-tauri/src/converter.rs` - `convert_single_file_with_retry`

**原因**: 文件转换任务内部可能发生 panic（如 unwrap 调用、数组越界等），导致整个程序崩溃。

**修复方案**:
```rust
// 使用 catch_unwind 捕获 panic，防止程序崩溃
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    tokio::runtime::Handle::current().block_on(async move {
        convert_single_file_with_retry(...)
        .await
    })
})).unwrap_or_else(|_| {
    safe_log!(error, "File {} conversion panicked, creating error result", file.name);
    ConversionResult {
        file_id: file.id.clone(),
        success: false,
        output_path: None,
        error: Some("Conversion panicked (internal error)".to_string()),
    }
});
```

**效果**: 即使某个文件转换任务 panic，程序也不会崩溃，而是记录错误并继续处理其他文件。

#### 问题 1.2: 信号量获取超时
**位置**: `src-tauri/src/converter.rs:324-338`

**原因**: 信号量获取可能永久阻塞或失败，导致死锁。

**修复方案**:
```rust
// 添加超时和错误处理
let permit = match tokio::time::timeout(
    tokio::time::Duration::from_secs(60),
    semaphore.clone().acquire_owned()
).await {
    Ok(Ok(p)) => p,
    Ok(Err(_)) => {
        log::error!("[Converter] Failed to acquire semaphore permit");
        continue;  // 跳过此文件，继续处理下一个
    }
    Err(_) => {
        log::warn!("[Converter] Semaphore acquisition timeout, skipping file");
        continue;
    }
};
```

**效果**: 超时后自动跳过该文件，避免无限等待。

#### 问题 1.3: 路径处理失败
**位置**: `src-tauri/src/converter.rs:618`

**原因**: `strip_prefix` 可能失败导致 panic。

**修复方案**:
```rust
let relative_path = match source_path.strip_prefix(base_dir) {
    Ok(path) => path,
    Err(_) => {
        safe_log!(warn, "Source path not in base directory, using full path: {}", file.path);
        source_path
    }
};
```

**效果**: 路径处理失败时使用安全默认值，避免 panic。

### 2. 改进的日志记录

#### 2.1 新增日志目录管理

**新增功能**:
- `AppState` 新增 `log_dir` 字段，在应用启动时初始化
- 添加 `get_log_directory` 命令，获取日志目录路径
- 添加 `open_log_directory` 命令，直接打开日志目录

**实现代码**:
```rust
pub struct AppState {
    // ... 其他字段
    pub log_dir: Arc<Mutex<Option<PathBuf>>>,  // 日志目录路径
}
```

```rust
#[tauri::command]
async fn get_log_directory(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let log_dir = state.log_dir.lock().await;
    if let Some(path) = log_dir.as_ref() {
        Ok(path.to_string_lossy().to_string())
    } else {
        Err("Log directory not initialized".to_string())
    }
}

#[tauri::command]
async fn open_log_directory(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // 打开日志目录
    // ...
}
```

#### 2.2 增强日志格式

**改进点**:
- 日志消息增加位置信息 `[文件名:行号]`
- 所有模块使用统一的日志前缀
- 错误级别使用 `safe_log!` 宏确保记录

**改进后的格式**:
```
[2025-03-10 14:23:45] [ERROR] [converter][converter.rs:1012] Failed to spawn FFmpeg: ...
```

#### 2.3 日志存储位置

**应用日志目录**:
- **Windows**: `%APPDATA%\com.bilibili.converter\logs\`
  - 完整路径: `C:\Users\<用户名>\AppData\Roaming\com.bilibili.converter\logs\`
- **macOS**: `~/Library/Logs/com.bilibili.converter/`
- **Linux**: `~/.local/share/com.bilibili.converter/logs/`

**日志文件命名**: `bilibili-converter-YYYY-MM-DD.log`

### 3. 前端集成建议

在前端添加查看日志的功能：

```typescript
// 获取日志目录
const logDir = await invoke('get_log_directory');
console.log('日志目录:', logDir);

// 打开日志目录
await invoke('open_log_directory');
```

**建议 UI 改进**:
1. 在设置页面添加"查看日志"按钮
2. 在关于页面显示日志路径
3. 发生错误时提供"打开日志"选项

## 防崩溃措施总结

### 已实现的保护措施

1. **任务级 Panic 捕获**: 使用 `std::panic::catch_unwind` 捕获所有文件转换任务中的 panic
2. **信号量超时保护**: 为信号量获取添加 60 秒超时
3. **路径安全处理**: 所有路径操作使用 `match` 而非 `unwrap`
4. **错误传播**: 所有错误都返回 `ConversionResult` 而非 panic
5. **进程清理**: 取消转换时正确清理 FFmpeg 子进程

### 日志覆盖的关键操作

1. **应用启动**: 记录启动时间和日志目录
2. **文件扫描**: 记录扫描的文件和发现的媒体文件
3. **转换开始**: 记录 FFmpeg 路径和编码器配置
4. **转换进度**: 记录每个文件的转换进度
5. **转换错误**: 记录所有转换失败的详细原因
6. **GPU 检测**: 记录可用的 GPU 加速编码器
7. **进程管理**: 记录 FFmpeg 进程的创建和清理

## 测试建议

### 崩溃测试

1. **并发压力测试**: 
   - 选择 50+ 文件同时转换
   - 观察是否崩溃

2. **异常输入测试**:
   - 选择包含特殊字符的文件路径
   - 选择包含大量子目录的文件夹
   - 选择包含损坏文件的文件夹

3. **中断测试**:
   - 转换过程中取消转换
   - 转换过程中暂停/恢复
   - 转换过程中关闭应用

### 日志验证

1. **检查日志文件生成**:
   ```powershell
   # Windows
   explorer $env:APPDATA\com.bilibili.converter\logs\
   ```

2. **验证日志内容**:
   - 应用启动时是否有日志
   - 转换操作是否记录
   - 错误是否详细记录

3. **测试日志命令**:
   ```typescript
   // 前端测试
   await invoke('get_log_directory');  // 应返回日志路径
   await invoke('open_log_directory');  // 应打开文件管理器
   ```

## 已知限制

1. **日志轮转**: 当前未实现日志文件大小限制和轮转
2. **性能日志**: 未添加性能指标日志（内存、CPU 使用率）
3. **结构化日志**: 当前使用纯文本日志，非 JSON 格式

## 未来改进方向

1. **崩溃恢复**: 记录转换进度到文件，重启后可恢复
2. **详细错误报告**: 前端显示错误时附带日志位置
3. **性能监控**: 添加系统资源使用日志
4. **日志过滤**: 前端可按级别筛选日志
5. **自动错误上报**: 可选的崩溃日志自动上传

## 技术细节

### Panic 机制

Rust 的 panic 机制：
- 默认情况下，panic 会展开栈并终止线程
- 在 `catch_unwind` 中，panic 被捕获为 `Box<Any>`
- 主线程中的 panic 会导致程序退出
- 子线程中的 panic 不会导致程序退出（除非使用 `join`）

我们的修复确保所有任务都在子线程中执行，并使用 `catch_unwind` 捕获 panic。

### 日志级别

使用建议：
- **ERROR**: 严重错误，需要立即关注
- **WARN**: 潜在问题，程序可继续运行
- **INFO**: 重要操作（开始转换、完成转换）
- **DEBUG**: 详细的调试信息（每个文件的进度）
- **TRACE**: 最详细的执行流程（每个函数调用）

## 相关文档

- [LOGGING_GUIDE.md](./LOGGING_GUIDE.md) - 日志配置和使用指南
- [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md) - 技术架构
- [API_DOCUMENTATION.md](./API_DOCUMENTATION.md) - API 文档

## 版本信息

- **修复版本**: 1.0.1
- **修复日期**: 2025-03-10
- **修改文件**:
  - `src-tauri/src/lib.rs` - 添加日志目录管理和新命令
  - `src-tauri/src/converter.rs` - 添加 panic 捕获和错误处理
