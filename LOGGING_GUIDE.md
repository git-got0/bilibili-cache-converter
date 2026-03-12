# 日志配置指南

## 概述

应用程序已配置日志记录功能,所有日志会自动保存到系统日志目录中,便于调试和追踪闪退问题。

## 日志配置

### 配置位置
- **文件**: `src-tauri/src/lib.rs` (行 653-672)
- **日志级别**:
  - 全局: `INFO`
  - 核心模块: `DEBUG` (`bilibili_converter`, `converter`, `scanner`)
- **日志格式**: `[时间戳] [级别] [模块] 消息`
- **输出目标**: 文件日志

## 日志存储位置

根据操作系统的不同,日志文件存储在不同位置:

### Windows
```
%APPDATA%\com.bilibili.converter\logs\
```
完整路径示例:
```
C:\Users\<用户名>\AppData\Roaming\com.bilibili.converter\logs\
```

### macOS
```
~/Library/Logs/com.bilibili.converter/
```

### Linux
```
~/.local/share/com.bilibili.converter/logs/
```

## 查看日志

### Windows 方法

#### 方法 1: 使用 PowerShell
```powershell
# 打开日志目录
explorer $env:APPDATA\com.bilibili.converter\logs\

# 查看最新的日志文件
Get-ChildItem $env:APPDATA\com.bilibili.converter\logs\ -Filter "*.log" | Sort-Object LastWriteTime -Descending | Select-Object -First 1 | Get-Content -Tail 50
```

#### 方法 2: 使用文件资源管理器
1. 按 `Win + R` 打开运行对话框
2. 输入 `%APPDATA%\com.bilibili.converter\logs\`
3. 按 Enter,会自动打开日志目录
4. 使用文本编辑器打开最新的 `.log` 文件

### 通用方法

在应用运行时,日志会实时写入文件。闪退发生时,最后几行日志通常包含错误信息。

## 日志级别说明

| 级别 | 说明 | 用途 |
|-----|------|-----|
| ERROR | 错误信息 | 需要立即关注的严重问题 |
| WARN | 警告信息 | 潜在问题,但程序仍可运行 |
| INFO | 一般信息 | 重要操作和状态变更 |
| DEBUG | 调试信息 | 详细的运行过程信息 |
| TRACE | 追踪信息 | 最详细的执行流程 |

## 崩溃分析

当应用闪退时,请按照以下步骤分析日志:

### 1. 找到最新的日志文件
日志文件按日期命名,格式为 `bilibili-converter-YYYY-MM-DD.log`

### 2. 查找错误信息
```bash
# Windows PowerShell
Select-String -Path "*.log" -Pattern "ERROR|panic|unwrap|expect" -Context 2,2
```

### 3. 关键错误模式

#### Rust Panic
```
[2025-03-09 14:23:45] [ERROR] [bilibili_converter] thread 'main' panicked at ...
```

#### Unwrap/Expect 错误
```
[2025-03-09 14:23:45] [ERROR] [converter] called `Result::unwrap()` on an `Err` value
```

#### Async 错误
```
[2025-03-09 14:23:45] [ERROR] [scanner] Task join error
```

### 4. 上下文分析

查看错误发生前后的日志行,了解:
- 正在执行的操作
- 处理的文件路径
- 涉及的模块和函数
- 时间戳和顺序

## 常见崩溃原因

根据代码审查,以下是最可能的崩溃原因:

### 1. 信号量获取失败
**位置**: `src-tauri/src/converter.rs:324`
**原因**: `semaphore.acquire_owned().await.unwrap()` 在异常情况下会 panic
**日志特征**: 包含 `ERROR` 和 `semaphore` 关键词

### 2. 正则表达式编译失败
**位置**: `src-tauri/src/converter.rs:28-32`
**原因**: 正则表达式语法错误导致编译失败
**日志特征**: 包含 `regex` 和 `unwrap` 关键词

### 3. 路径处理错误
**位置**: `src-tauri/src/scanner.rs:115`
**原因**: 文件路径处理可能返回无效路径
**日志特征**: 包含 `parent` 和 `unwrap_or` 关键词

## 日志调试建议

### 开发环境
如需同时查看终端输出,修改日志配置:
```rust
.targets([
    tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::Stdout
    ),
    tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::LogDir { file_name: None }
    ),
])
```

### 生产环境
保持仅文件日志配置,避免性能影响。

## 崩溃报告模板

如果应用闪退,请提供以下信息以便快速定位问题:

```
### 基本信息
- 操作系统: Windows 10/11
- 应用版本: 1.0.0
- 崩溃时间: YYYY-MM-DD HH:MM:SS

### 日志片段
[粘贴最后 50-100 行日志,特别关注 ERROR 和 WARN 级别]

### 复现步骤
1. ...
2. ...
3. ...

### 预期行为
...

### 实际行为
...
```

## 相关文档

- [CRASH_FIX_REPORT.md](./CRASH_FIX_REPORT.md) - 已知的崩溃问题修复报告
- [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md) - 技术架构文档
- [API_DOCUMENTATION.md](./API_DOCUMENTATION.md) - API 文档
