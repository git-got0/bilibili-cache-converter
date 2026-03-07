# Bilibili缓存转换器 - API接口文档

## 目录
- [前端到后端接口 (Tauri Commands)](#前端到后端接口-tauri-commands)
- [后端到前端接口 (Tauri Events)](#后端到前端接口-tauri-events)
- [类型定义](#类型定义)
- [错误处理](#错误处理)

---

## 前端到后端接口 (Tauri Commands)

### 1. scan_folder

扫描指定文件夹,识别Bilibili缓存文件。

**命令路径**: `scan_folder`

**参数**:
```typescript
interface ScanFolderParams {
  folder_path: string;
}
```

**返回值**:
```typescript
interface ScanResult {
  files: MediaFile[];
  total_size: number;
}
```

**示例**:
```typescript
const result = await invoke<ScanResult>("scan_folder", {
  folder_path: "C:\\Bilibili\\Cache"
});
console.log(`找到 ${result.files.length} 个文件`);
```

**实现位置**: `src-tauri/src/lib.rs` 第96-101行

---

### 2. start_conversion

启动批量转换任务。

**命令路径**: `start_conversion`

**参数**:
```typescript
interface StartConversionParams {
  files: MediaFile[];
}
```

**返回值**: `void`

**触发事件**:
- `conversion-progress` - 转换进度更新
- `conversion-complete` - 转换完成

**示例**:
```typescript
await invoke("start_conversion", {
  files: scannedFiles
});
```

**实现位置**: `src-tauri/src/lib.rs` 第104-160行

**注意事项**:
- 转换任务在后台异步执行
- 同时只能有一个转换任务运行
- 会重置 `completed_count` 计数器

---

### 3. cancel_conversion

取消正在进行的转换任务。

**命令路径**: `cancel_conversion`

**参数**: 无

**返回值**:
```typescript
interface ConversionCancelledEvent {
  completed_count: number;
  total_count: number;
}
```

**触发事件**: `conversion-cancelled`

**示例**:
```typescript
const result = await invoke<ConversionCancelledEvent>("cancel_conversion");
console.log(`已完成 ${result.completed_count} 个文件`);
```

**实现位置**: `src-tauri/src/lib.rs` 第176-202行

**副作用**:
- 停止所有正在进行的转换任务
- 清空转换任务队列
- 将 `is_converting` 标志设为 false

---

### 4. get_settings

获取应用设置。

**命令路径**: `get_settings`

**参数**: 无

**返回值**:
```typescript
interface AppSettings {
  sound_enabled: boolean;         // 是否启用提示音
  output_format_video: string;    // 视频输出格式 (mp4/mkv/avi)
  output_format_audio: string;    // 音频输出格式 (mp3/aac/flac)
  output_path: string;            // 自定义输出路径
  concurrency: number;            // 并发数 (1/2/4/8)
}
```

**示例**:
```typescript
const settings = await invoke<AppSettings>("get_settings");
console.log(`当前视频格式: ${settings.output_format_video}`);
```

**实现位置**: `src-tauri/src/lib.rs` 第205-208行

---

### 5. update_settings

更新应用设置。

**命令路径**: `update_settings`

**参数**:
```typescript
interface UpdateSettingsParams {
  newSettings: AppSettings;
}
```

**返回值**: `void`

**示例**:
```typescript
await invoke("update_settings", {
  newSettings: {
    ...settings,
    sound_enabled: false,
    concurrency: 2
  }
});
```

**实现位置**: `src-tauri/src/lib.rs` 第211-219行

**注意事项**:
- 设置会自动保存到本地存储
- 部分设置(如并发数)在转换期间不可修改

---

### 6. open_output_folder

打开指定文件夹(使用系统文件管理器)。

**命令路径**: `open_output_folder`

**参数**:
```typescript
interface OpenOutputFolderParams {
  folder_path: string;
}
```

**返回值**: `void`

**平台差异**:
- **Windows**: 使用 `explorer /select,` 命令
- **macOS**: 使用 `open` 命令
- **Linux**: 使用 `xdg-open` 命令

**示例**:
```typescript
await invoke("open_output_folder", {
  folder_path: "C:\\Bilibili\\Output"
});
```

**实现位置**: `src-tauri/src/lib.rs` 第222-232行

**安全验证** (v1.0.2+):
- 验证路径为绝对路径
- 验证路径存在且为目录
- 防止路径遍历攻击

---

### 7. ensure_output_directory

确保输出目录存在,不存在则创建。

**命令路径**: `ensure_output_directory`

**参数**:
```typescript
interface EnsureOutputDirectoryParams {
  path: string;
}
```

**返回值**: `void`

**示例**:
```typescript
await invoke("ensure_output_directory", {
  path: "C:\\Bilibili\\Output"
});
```

**实现位置**: `src-tauri/src/lib.rs` 第235-239行

**注意事项**:
- 使用 `std::fs::create_dir_all` 递归创建目录
- 如果目录已存在,不会报错

**安全验证** (v1.0.2+):
- 验证路径为绝对路径
- 防止路径遍历攻击

---

### 8. get_default_output_path

根据源文件夹路径计算默认输出路径。

**命令路径**: `get_default_output_path`

**参数**:
```typescript
interface GetDefaultOutputPathParams {
  folder_path: string;
}
```

**返回值**: `string`

**逻辑**: 默认路径 = 源文件夹路径 + "/result"

**示例**:
```typescript
const defaultPath = await invoke<string>("get_default_output_path", {
  folder_path: "C:\\Bilibili\\Cache"
});
console.log(`默认输出路径: ${defaultPath}`); // 输出: C:\Bilibili\Cache\result
```

**实现位置**: `src-tauri/src/lib.rs` 第247-258行

---

### 9. get_ffmpeg_path

获取FFmpeg可执行文件路径。

**命令路径**: `get_ffmpeg_path`

**参数**: 无

**返回值**: `string`

**实现位置**: `src-tauri/src/lib.rs` 第242-244行

**实现细节**: 实际实现在 `src-tauri/src/converter.rs`

---

### 10. pause_conversion

暂停正在进行的转换任务。

**命令路径**: `pause_conversion`

**参数**: 无

**返回值**:
```typescript
interface ConversionPausedEvent {
  completed_count: number;  // 已完成数量
  pending_count: number;    // 待处理数量
}
```

**触发事件**: `conversion-paused`

**示例**:
```typescript
const result = await invoke<ConversionPausedEvent>("pause_conversion");
console.log(`已暂停,已完成 ${result.completed_count} 个文件`);
```

**实现位置**: `src-tauri/src/lib.rs` 第287-331行

**注意事项**:
- 仅在有转换任务运行时可用
- 已暂停时再次调用会报错
- 转换任务不会被取消,只是暂停处理

---

### 11. resume_conversion

恢复已暂停的转换任务。

**命令路径**: `resume_conversion`

**参数**: 无

**返回值**:
```typescript
interface ConversionResumedEvent {
  completed_count: number;  // 已完成数量
  pending_count: number;    // 待处理数量
}
```

**触发事件**: `conversion-resumed`

**示例**:
```typescript
const result = await invoke<ConversionResumedEvent>("resume_conversion");
console.log(`已恢复,待处理 ${result.pending_count} 个文件`);
```

**实现位置**: `src-tauri/src/lib.rs` 第340-389行

**注意事项**:
- 仅在转换任务已暂停时可用
- 未暂停时调用会报错

---

## 后端到前端接口 (Tauri Events)

### 1. conversion-progress

转换进度更新事件。

**事件名称**: `conversion-progress`

**事件数据**:
```typescript
interface ConversionProgress {
  file_id: string;           // 当前文件ID
  file_name: string;         // 当前文件名
  progress: number;           // 当前进度 (0-100)
  status: string;            // 状态描述
  current_index: number;     // 当前文件索引 (从0开始)
  total_count: number;       // 总文件数
}
```

**整体进度计算公式**:
```typescript
const overallProgress = (progress.current_index * 100 + progress.progress) / progress.total_count;
```

**触发时机**:
- 每个文件转换过程中
- 进度值变化时(每10%或根据实现)

**监听示例**:
```typescript
const unlisten = await listen<ConversionProgress>("conversion-progress", (event) => {
  const progress = event.payload;
  const overall = (progress.current_index * 100 + progress.progress) / progress.total_count;
  console.log(`总体进度: ${Math.round(overall)}%`);
  console.log(`当前文件: ${progress.current_index + 1}/${progress.total_count}`);
});
```

**实现位置**: `src-tauri/src/converter.rs` - `convert_single_file` 函数

---

### 2. conversion-complete

转换完成事件。

**事件名称**: `conversion-complete`

**事件数据**:
```typescript
interface ConversionCompleteEvent {
  success_count: number;     // 成功转换数量
  total_count: number;       // 总文件数
  results: ConversionResult[];
}

interface ConversionResult {
  file_id: string;          // 文件ID
  success: boolean;          // 是否成功
  output_path?: string;      // 输出路径(成功时)
  error?: string;           // 错误信息(失败时)
}
```

**触发时机**:
- 所有文件转换完成后
- 用户取消转换时

**监听示例**:
```typescript
const unlisten = await listen<ConversionCompleteEvent>("conversion-complete", (event) => {
  const { success_count, total_count, results } = event.payload;
  console.log(`转换完成: ${success_count}/${total_count}`);
  results.forEach(result => {
    if (!result.success) {
      console.error(`失败: ${result.file_id} - ${result.error}`);
    }
  });
});
```

**实现位置**: `src-tauri/src/lib.rs` 第145-156行

---

### 3. conversion-cancelled

转换取消事件。

**事件名称**: `conversion-cancelled`

**事件数据**:
```typescript
interface ConversionCancelledEvent {
  completed_count: number;   // 已完成数量
  total_count: number;       // 总文件数
}
```

**触发时机**:
- 用户调用 `cancel_conversion` 命令时

**监听示例**:
```typescript
const unlisten = await listen<ConversionCancelledEvent>("conversion-cancelled", (event) => {
  const { completed_count, total_count } = event.payload;
  console.log(`转换已取消,已完成: ${completed_count}/${total_count}`);
});
```

**实现位置**: `src-tauri/src/lib.rs` 第198行

---

### 4. conversion-paused

转换暂停事件。

**事件名称**: `conversion-paused`

**事件数据**:
```typescript
interface ConversionPausedEvent {
  completed_count: number;  // 已完成数量
  pending_count: number;    // 待处理数量
}
```

**触发时机**:
- 用户调用 `pause_conversion` 命令成功时

**监听示例**:
```typescript
const unlisten = await listen<ConversionPausedEvent>("conversion-paused", (event) => {
  const { completed_count, pending_count } = event.payload;
  console.log(`转换已暂停,已完成: ${completed_count}, 待处理: ${pending_count}`);
});
```

**实现位置**: `src-tauri/src/lib.rs` 第327行

---

### 5. conversion-resumed

转换恢复事件。

**事件名称**: `conversion-resumed`

**事件数据**:
```typescript
interface ConversionResumedEvent {
  completed_count: number;  // 已完成数量
  pending_count: number;    // 待处理数量
}
```

**触发时机**:
- 用户调用 `resume_conversion` 命令成功时

**监听示例**:
```typescript
const unlisten = await listen<ConversionResumedEvent>("conversion-resumed", (event) => {
  const { completed_count, pending_count } = event.payload;
  console.log(`转换已恢复,已完成: ${completed_count}, 待处理: ${pending_count}`);
});
```

**实现位置**: `src-tauri/src/lib.rs` 第378-381行

---

### 6. play-notification-sound

播放通知声音事件。

**事件名称**: `play-notification-sound`

**事件数据**: 无

**触发时机**:
- 转换完成且 `sound_enabled` 为 true 时

**监听示例**:
```typescript
const unlisten = await listen("play-notification-sound", () => {
  // 播放提示音
  const audio = new Audio('/notification.mp3');
  audio.play();
});
```

**实现位置**: `src-tauri/src/lib.rs` 第154-156行

---

### 7. scan-progress

扫描进度事件。

**事件名称**: `scan-progress`

**事件数据**:
```typescript
interface ScanProgress {
  found_files: number;      // 已找到的文件数
  current_path: string;     // 当前扫描路径
}
```

**触发时机**:
- 扫描过程中(可选实现)

**监听示例**:
```typescript
const unlisten = await listen<ScanProgress>("scan-progress", (event) => {
  console.log(`扫描中...已找到 ${event.payload.found_files} 个文件`);
});
```

**实现位置**: `src-tauri/src/scanner.rs`

---

## 类型定义

### MediaFile

```typescript
interface MediaFile {
  id: string;                // 唯一标识符
  path: string;              // 文件完整路径
  name: string;              // 文件名
  size: number;              // 文件大小(字节)
  file_type: "video" | "audio";  // 文件类型
  title: string;             // 文件标题
  output_name: string;       // 输出文件名
  has_audio?: boolean;       // 是否包含音频(视频文件)
}
```

---

### ScanResult

```typescript
interface ScanResult {
  files: MediaFile[];        // 扫描到的文件列表
  total_size: number;       // 总大小(字节)
}
```

---

### AppSettings

```typescript
interface AppSettings {
  sound_enabled: boolean;         // 是否启用提示音
  output_format_video: string;    // 视频输出格式 (mp4/mkv/avi)
  output_format_audio: string;    // 音频输出格式 (mp3/aac/flac)
  output_path: string;            // 自定义输出路径
  concurrency: number;            // 并发数 (1/2/4/8)
}
```

---

### ConversionProgress

```typescript
interface ConversionProgress {
  file_id: string;           // 当前文件ID
  file_name: string;         // 当前文件名
  progress: number;         // 当前进度 (0-100)
  status: string;           // 状态描述
  current_index: number;     // 当前文件索引
  total_count: number;      // 总文件数
  elapsed_time: number;      // 已用时间(秒)
  remaining_time: number;    // 预计剩余时间(秒)
  // 性能指标
  conversion_speed: number;  // 当前转换速度(MB/s)
  average_speed: number;     // 平均转换速度(MB/s)
  estimated_size: number;    // 预计输出大小(字节)
  processed_bytes: number;  // 已处理字节数
}
```

**性能指标说明**:
- `conversion_speed`: 当前文件的实时转换速度
- `average_speed`: 所有已处理文件的平均转换速度
- `estimated_size`: 基于输入大小和格式估算的输出文件大小
- `processed_bytes`: 当前文件已处理的字节数

---

### ConversionResult

```typescript
interface ConversionResult {
  file_id: string;          // 文件ID
  success: boolean;         // 是否成功
  output_path?: string;     // 输出路径(成功时)
  error?: string;          // 错误信息(失败时)
}
```

---

### ConversionCompleteEvent

```typescript
interface ConversionCompleteEvent {
  success_count: number;      // 成功转换数量
  total_count: number;        // 总文件数
  results: ConversionResult[];
}
```

---

### ConversionCancelledEvent

```typescript
interface ConversionCancelledEvent {
  completed_count: number;   // 已完成数量
  total_count: number;       // 总文件数
}
```

---

### ConversionPausedEvent

```typescript
interface ConversionPausedEvent {
  completed_count: number;  // 已完成数量
  pending_count: number;    // 待处理数量
}
```

---

### ConversionResumedEvent

```typescript
interface ConversionResumedEvent {
  completed_count: number;  // 已完成数量
  pending_count: number;    // 待处理数量
}
```

---

### ScanProgress

```typescript
interface ScanProgress {
  found_files: number;      // 已找到的文件数
  current_path: string;     // 当前扫描路径
}
```

---

## 错误处理

### 错误传递机制

Tauri使用 Rust 的 `Result<T, E>` 类型处理错误,错误会自动转换为 JavaScript 的 `Promise.reject`。

### 错误捕获示例

```typescript
try {
  await invoke("some_command", { param: "value" });
} catch (error) {
  console.error("命令执行失败:", error);
  // error 是一个字符串,包含错误信息
}
```

### 常见错误

| 错误信息 | 原因 | 解决方案 |
|---------|------|---------|
| "FFmpeg not found" | FFmpeg未安装或未在PATH中 | 安装FFmpeg并添加到系统PATH |
| "Conversion already in progress" | 已有转换任务在运行 | 等待当前任务完成或取消 |
| "Failed to scan folder" | 文件夹路径无效或权限不足 | 检查路径和权限 |
| "Failed to convert file" | 文件转换失败 | 检查源文件完整性 |

### 错误处理最佳实践

```typescript
const handleOperation = async () => {
  try {
    setError(null);
    await invoke("some_command", { param: value });
  } catch (err) {
    console.error("操作失败:", err);
    setError("操作失败: " + err);
    toast.error("操作失败", {
      description: String(err)
    });
  }
};
```

---

## 安全性考虑

### 输入验证

1. **路径验证**: 所有文件路径在Rust端进行验证
2. **类型检查**: TypeScript编译时类型检查
3. **权限控制**: Tauri配置中限制文件访问范围

### 敏感操作

- 文件系统操作受Tauri权限系统保护
- 外部命令执行(FFmpeg)受控

---

## 性能优化

### 事件频率控制

- 转换进度事件不应过于频繁(建议间隔≥100ms)
- 使用防抖/节流减少UI更新频率

### 并发控制

- 根据系统资源合理设置并发数
- 避免过多并发导致系统卡顿

---

**文档版本**: 1.0.0
**最后更新**: 2026年3月7日
