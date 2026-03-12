# 高级日志系统指南

本文档介绍 Bilibili 缓存转换器的详细日志记录功能。

## 功能特性

### 1. 日志级别

支持 5 个日志级别，按严重程度排序：

| 级别 | 说明 | 使用场景 |
|------|------|----------|
| `TRACE` | 最详细的日志 | 开发调试时追踪程序执行流程 |
| `DEBUG` | 调试信息 | 开发和测试阶段的详细信息 |
| `INFO` | 一般信息 | 程序运行的关键事件（默认级别） |
| `WARN` | 警告信息 | 潜在问题，但不影响程序运行 |
| `ERROR` | 错误信息 | 严重错误，需要关注和处理 |

### 2. 日志文件管理

#### 文件命名规则
```
bilibili-converter-YYYY-MM-DD.log         # 主日志文件
bilibili-converter-YYYY-MM-DD-N.log       # 大小分割后的序列文件
```

#### 自动分割机制
- **按日期分割**: 每天自动创建新的日志文件
- **按大小分割**: 单个文件超过 10MB 时自动创建新文件
- **文件数量限制**: 默认保留最近 30 个日志文件

### 3. 日志格式

每条日志包含以下信息：
```
[时间戳] [级别] [模块名] [线程ID] [源码位置] 日志内容
```

示例：
```
[2024-03-15 14:30:25] [INFO ] [startup] [T1] [lib.rs:82] 应用启动
[2024-03-15 14:30:25] [INFO ] [startup] [T1] [lib.rs:83] 默认日志目录: C:\Users\xxx\AppData\Roaming\com.bilibili-converter.app
[2024-03-15 14:30:30] [DEBUG] [scanner] [T2] [scanner.rs:112] Found media file: video.m4s (type: Some("video"))
[2024-03-15 14:31:00] [ERROR] [converter] [T3] [converter.rs:505] Conversion failed: FFmpeg not found
```

## API 接口

### 前端调用示例

```typescript
import { invoke } from "@tauri-apps/api/core";

// 获取日志统计信息
const stats = await invoke<LoggerStats>("get_logger_stats");
console.log("日志目录:", stats.log_directory);
console.log("日志条目数:", stats.total_entries);
console.log("错误数:", stats.error_count);

// 设置日志级别
await invoke("set_log_level", { level: "Debug" });

// 获取当前日志级别
const level = await invoke<string>("get_log_level");

// 读取最近的日志
const logs = await invoke<string[]>("read_recent_logs", { lines: 100 });
logs.forEach(line => console.log(line));

// 手动刷新日志到磁盘
await invoke("flush_logs");
```

### 后端调用示例

```rust
use crate::logger::{self, LogLevel};

// 简单日志
logger::log(LogLevel::Info, "module_name", "操作成功完成", None);

// 带位置的日志（通常使用宏）
log_info!("converter", "转换完成: {} -> {}", input, output);
log_error!("scanner", "扫描失败: {}", error);
log_warn!("settings", "配置项缺失，使用默认值");
log_debug!("converter", "处理进度: {}%", progress);
```

## 日志文件位置

### 默认位置
- Windows: `%APPDATA%\com.bilibili-converter.app\`
- macOS: `~/Library/Application Support/com.bilibili-converter.app/`
- Linux: `~/.config/com.bilibili-converter.app/`

### 用户自定义位置
当用户设置输出目录后，日志自动移动到 `<输出目录>/logs/` 目录。

## 配置选项

```rust
LoggerConfig {
    log_dir: PathBuf,           // 日志目录路径
    min_level: LogLevel,        // 最小日志级别
    max_file_size: u64,         // 单文件最大大小（字节）
    max_files: usize,           // 最大文件数量
    include_thread_id: bool,    // 是否包含线程ID
    include_location: bool,     // 是否包含源码位置
}
```

## 性能特性

- **异步写入**: 日志写入不阻塞主线程
- **批量刷新**: 减少磁盘 I/O 操作
- **错误自动提升**: WARN 和 ERROR 级别立即刷新
- **线程安全**: 支持多线程并发写入

## 故障排查

### 日志文件未生成
1. 检查日志目录权限
2. 确认日志级别设置正确
3. 查看控制台是否有错误信息

### 日志文件过大
1. 调整 `max_file_size` 参数
2. 提高日志级别（如从 DEBUG 改为 INFO）
3. 减少 `max_files` 数量

### 日志丢失
1. 程序崩溃前调用 `flush_logs()`
2. ERROR 级别日志会自动立即刷新
3. 正常退出时会自动刷新所有日志

## 最佳实践

1. **开发阶段**: 使用 `DEBUG` 或 `TRACE` 级别
2. **生产环境**: 使用 `INFO` 级别
3. **问题排查**: 临时启用 `DEBUG` 级别
4. **关键操作**: 使用 `INFO` 记录
5. **错误处理**: 使用 `ERROR` 记录并包含详细信息
6. **性能监控**: 使用 `DEBUG` 记录耗时操作

## 日志分析

可以使用任何文本编辑器或日志分析工具查看日志文件：

```bash
# 查看最新日志
tail -f bilibili-converter-*.log

# 搜索错误
grep "\[ERROR\]" bilibili-converter-*.log

# 统计错误数量
grep -c "\[ERROR\]" bilibili-converter-*.log

# 按时间过滤
grep "2024-03-15 14:" bilibili-converter-*.log
```
