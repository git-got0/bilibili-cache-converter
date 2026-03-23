use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State,
};
use tokio::sync::Mutex;

mod converter;
mod logger;
mod scanner;

// IntegrityValidation is defined in this file (line ~18), not in converter module

/// Integrity validation result for converted files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityValidation {
    pub file_id: String,
    pub is_valid: bool,
    pub validation_details: Vec<String>,
    pub file_size: u64,
    pub expected_size: Option<u64>,
}

/// Determine the best log directory:
/// 1. Try program installation directory (resource_dir/logs)
/// 2. Fall back to AppData if not writable
fn determine_log_directory(app: &tauri::App) -> std::path::PathBuf {
    use std::fs;

    // Try resource directory (installation directory) first
    if let Ok(resource_dir) = app.path().resource_dir() {
        let log_dir = resource_dir.join("logs");

        // Try to create the directory
        if fs::create_dir_all(&log_dir).is_ok() {
            // Try to write a test file to verify permissions
            let test_file = log_dir.join(".write_test");
            if fs::write(&test_file, "test").is_ok() {
                let _ = fs::remove_file(test_file);
                return log_dir;
            }
        }
    }

    // Fallback: use AppData directory
    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let log_dir = app_data_dir.join("logs");
        if fs::create_dir_all(&log_dir).is_ok() {
            return log_dir;
        }
    }

    // Last resort: use current directory - 但添加诊断信息
    let fallback = std::path::PathBuf::from("logs");
    eprintln!(
        "[ERROR] 无法创建日志目录到 resource_dir 或 AppData，\
         将使用回退目录: {}",
        fallback.display()
    );
    fallback
}

/// Generate a simple timestamp in format: YYYY-MM-DD HH:MM:SS
fn chrono_lite_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Calculate year, month, day from days since epoch (1970-01-01)
    let mut year = 1970;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, seconds
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: String,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub file_type: String,
    pub title: String,
    pub output_name: String,
    pub has_audio: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionProgress {
    pub file_id: String,
    pub file_name: String,
    pub progress: f64,
    pub status: String,
    pub current_index: usize,
    pub completed_count: usize,
    pub total_count: usize,
    pub elapsed_time: u64,
    pub remaining_time: u64,
    // Performance metrics
    pub conversion_speed: f64, // MB/s
    pub average_speed: f64,    // Average MB/s
    pub estimated_size: u64,   // Estimated output size in bytes
    pub processed_bytes: u64,  // Bytes processed so far
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub file_id: String,
    pub success: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub files: Vec<MediaFile>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub processed: u64,  // 已处理的文件数
    pub total: u64,      // 总文件数（0 表示未知）
    pub message: String, // 进度消息
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub sound_enabled: bool,
    pub output_format_video: String,
    pub output_format_audio: String,
    pub output_path: String,
    pub concurrency: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        let cpu_count = num_cpus::get();
        // 选择最接近且不超过 CPU 核心数的有效并发值 [1, 2, 4, 6, 8]
        let valid_concurrencies = [1usize, 2, 4, 6, 8];
        let concurrency = valid_concurrencies
            .iter()
            .rev()
            .find(|&&v| v <= cpu_count)
            .copied()
            .unwrap_or(1);
        Self {
            sound_enabled: true,
            output_format_video: "mp4".to_string(),
            output_format_audio: "mp3".to_string(),
            output_path: String::new(),
            concurrency,
        }
    }
}

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub conversion_tasks: Mutex<HashMap<String, converter::ConversionTask>>,
    pub is_converting: Mutex<bool>,
    pub is_paused: Mutex<bool>,
    pub completed_count: Mutex<usize>,
    pub start_time: Mutex<Option<std::time::Instant>>,
    pub pending_files: Mutex<Vec<MediaFile>>, // Files pending to be processed after resume
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(AppSettings::default()),
            conversion_tasks: Mutex::new(HashMap::new()),
            is_converting: Mutex::new(false),
            is_paused: Mutex::new(false),
            completed_count: Mutex::new(0),
            start_time: Mutex::new(None),
            pending_files: Mutex::new(Vec::new()),
        }
    }
}

#[tauri::command]
async fn scan_folder(app: AppHandle, folder_path: String) -> Result<ScanResult, String> {
    // 使用 eprintln 保证关键信息始终可见
    eprintln!("[命令] scan_folder 被调用");
    eprintln!("[参数] folder_path: {}", folder_path);

    // DISABLED: 日志系统已注释，仅使用 eprintln 调试
    // log::info!("[scan_folder] 开始扫描文件夹：{}", folder_path);

    // 路径验证
    let path = std::path::Path::new(&folder_path);
    if !path.exists() {
        let error_msg = format!("文件夹不存在：{}", folder_path);
        eprintln!("[错误] {}", error_msg);
        // log::error!("[scan_folder] {}", error_msg);  // DISABLED
        return Err(error_msg);
    }

    if !path.is_dir() {
        let error_msg = format!("路径不是文件夹：{}", folder_path);
        eprintln!("[错误] {}", error_msg);
        // log::error!("[scan_folder] {}", error_msg);  // DISABLED
        return Err(error_msg);
    }

    // 检查路径长度（Windows 最大 260 字符）
    if folder_path.len() > 240 {
        let error_msg = format!("路径过长 ({} 字符): {}", folder_path.len(), folder_path);
        eprintln!("[错误] {}", error_msg);
        // log::error!("[scan_folder] {}", error_msg);  // DISABLED
        return Err(error_msg);
    }

    // log::info!("[scan_folder] 路径验证通过，调用 scanner 模块");  // DISABLED

    // 直接调用，不使用 catch_unwind（因为 async 函数的限制）
    match scanner::scan_bilibili_files(&folder_path, Some(app)).await {
        Ok(result) => {
            eprintln!("[成功] 扫描完成，找到 {} 个文件", result.files.len());
            // log::info!(...)  // DISABLED
            Ok(result)
        }
        Err(e) => {
            let error_msg = format!("扫描失败：{}", e);
            eprintln!("[错误] {}", error_msg);
            // log::error!("[scan_folder] {}", error_msg);  // DISABLED
            Err(error_msg)
        }
    }
}

#[tauri::command]
async fn start_conversion(
    app: AppHandle,
    files: Vec<MediaFile>,
    folder_path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    eprintln!("[命令] start_conversion 被调用");
    eprintln!("[参数] files: {} 个文件", files.len());
    eprintln!("[参数] folder_path: {}", folder_path);

    // 直接执行，不使用 catch_unwind（因为 async 闭包的问题）
    // Reset completed count
    {
        let mut count = state.completed_count.lock().await;
        *count = 0;
    }

    // Record start time
    {
        let mut start_time = state.start_time.lock().await;
        *start_time = Some(std::time::Instant::now());
    }

    let settings = state.settings.lock().await.clone();
    let is_converting = state.is_converting.lock().await;

    if *is_converting {
        return Err("Conversion already in progress".to_string());
    }
    drop(is_converting);

    let mut is_converting = state.is_converting.lock().await;
    *is_converting = true;
    drop(is_converting);

    let app_clone = app.clone();
    let state_arc = state.inner().clone();

    tokio::spawn(async move {
        // Get start time from state
        let start_time = {
            let time = state_arc.start_time.lock().await;
            *time
        };

        let results = converter::convert_files(
            app_clone.clone(),
            files.clone(),
            &folder_path,
            &settings,
            state_arc.clone(),
            start_time,
        )
        .await;

        let mut is_converting = state_arc.is_converting.lock().await;
        *is_converting = false;

        let success_count = results.iter().filter(|r| r.success).count();
        let total = results.len();

        let _ = app_clone.emit(
            "conversion-complete",
            ConversionCompleteEvent {
                success_count,
                total_count: total,
                results,
            },
        );

        if settings.sound_enabled {
            let _ = app_clone.emit("play-notification-sound", ());
        }
    });

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionCompleteEvent {
    pub success_count: usize,
    pub total_count: usize,
    pub results: Vec<ConversionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionCancelledEvent {
    pub completed_count: usize,
    pub total_count: usize,
}

#[tauri::command]
async fn cancel_conversion(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ConversionCancelledEvent, String> {
    let completed_count = {
        let count = state.completed_count.lock().await;
        *count
    };

    let total_count = {
        let tasks = state.conversion_tasks.lock().await;
        tasks.len()
    };

    let mut is_converting = state.is_converting.lock().await;
    *is_converting = false;

    // Kill all running FFmpeg processes
    #[cfg(windows)]
    {
        use std::process::Command;
        // Kill all ffmpeg.exe and ffprobe.exe processes
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ffmpeg.exe"])
            .spawn();
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ffprobe.exe"])
            .spawn();
        eprintln!("[命令] 已终止所有 FFmpeg 进程");
    }

    let mut tasks = state.conversion_tasks.lock().await;
    tasks.clear();

    let event = ConversionCancelledEvent {
        completed_count,
        total_count,
    };

    let _ = app.emit("conversion-cancelled", event.clone());

    log::info!(
        "Conversion cancelled, completed: {}/{}",
        completed_count,
        total_count
    );
    Ok(event)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionPausedEvent {
    pub completed_count: usize,
    pub pending_count: usize,
}

#[tauri::command]
async fn pause_conversion(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ConversionPausedEvent, String> {
    let is_converting = {
        let converting = state.is_converting.lock().await;
        *converting
    };

    if !is_converting {
        return Err("No conversion in progress".to_string());
    }

    let is_paused = {
        let paused = state.is_paused.lock().await;
        *paused
    };

    if is_paused {
        return Err("Conversion already paused".to_string());
    }

    // Set paused state
    {
        let mut paused = state.is_paused.lock().await;
        *paused = true;
    }

    let completed_count = {
        let count = state.completed_count.lock().await;
        *count
    };

    let pending_count = {
        let tasks = state.conversion_tasks.lock().await;
        tasks.len()
    };

    let event = ConversionPausedEvent {
        completed_count,
        pending_count,
    };

    let _ = app.emit("conversion-paused", event.clone());

    eprintln!(
        "[命令] 转换已暂停，已完成：{}/{}",
        completed_count,
        completed_count + pending_count
    );
    Ok(event)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResumedEvent {
    pub completed_count: usize,
    pub pending_count: usize,
}

#[tauri::command]
async fn resume_conversion(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ConversionResumedEvent, String> {
    let is_converting = {
        let converting = state.is_converting.lock().await;
        *converting
    };

    if !is_converting {
        return Err("No conversion in progress".to_string());
    }

    let is_paused = {
        let paused = state.is_paused.lock().await;
        *paused
    };

    if !is_paused {
        return Err("Conversion is not paused".to_string());
    }

    // Clear paused state to resume
    {
        let mut paused = state.is_paused.lock().await;
        *paused = false;
    }

    let completed_count = {
        let count = state.completed_count.lock().await;
        *count
    };

    let pending_count = {
        let tasks = state.conversion_tasks.lock().await;
        tasks.len()
    };

    let _ = app.emit(
        "conversion-resumed",
        ConversionResumedEvent {
            completed_count,
            pending_count,
        },
    );

    eprintln!(
        "[命令] 转换已恢复，已完成：{}, 待处理：{}",
        completed_count, pending_count
    );
    Ok(ConversionResumedEvent {
        completed_count,
        pending_count,
    })
}

#[tauri::command]
async fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().await;
    Ok(settings.clone())
}

#[tauri::command]
async fn update_settings(
    new_settings: AppSettings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let mut settings = state.settings.lock().await;
    *settings = new_settings;
    eprintln!("[命令] 设置已更新");
    Ok(())
}

#[tauri::command]
async fn open_output_folder(folder_path: String) -> Result<(), String> {
    use std::path::Path;

    // Validate and sanitize the path to prevent directory traversal attacks
    let path = Path::new(&folder_path);
    if !path.is_absolute() {
        return Err("Invalid path: must be absolute path".to_string());
    }

    // Additional check: ensure path exists and is a directory
    if !path.exists() {
        return Err("Directory does not exist".to_string());
    }
    if !path.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn ensure_output_directory(path: String) -> Result<(), String> {
    use std::path::Path;

    // Validate and sanitize the path
    let path_obj = Path::new(&path);
    if !path_obj.is_absolute() {
        return Err("Invalid path: must be absolute path".to_string());
    }

    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    log::info!("Created output directory: {}", path);
    Ok(())
}

#[tauri::command]
async fn get_ffmpeg_path(app: AppHandle) -> Result<String, String> {
    converter::get_ffmpeg_path(Some(&app)).await
}

/// Get default output path with simplification rules applied
/// Used internally for conversion to avoid deep directory nesting
#[tauri::command]
async fn get_suggested_output_path(folder_path: String) -> Result<String, String> {
    use std::path::Path;
    // Validate input path
    let path = Path::new(&folder_path);
    if !path.is_absolute() {
        return Err("Invalid path: must be absolute path".to_string());
    }

    // Get parent directory and apply simplification rules
    let parent = path.parent().unwrap_or(path);

    let simplified = do_simplify_output_path(parent);

    let output_path = simplified.join("result").display().to_string();

    Ok(output_path)
}

/// Get suggested output path without simplification
/// Used for UI display to show the direct parent + result folder
#[tauri::command]
async fn get_default_output_path(folder_path: String) -> Result<String, String> {
    use std::path::Path;

    // Validate input path
    let path: &Path = Path::new(&folder_path);
    if !path.is_absolute() {
        return Err("Invalid path: must be absolute path".to_string());
    }

    // Directly create result folder under the given path
    let output_path = path.join("result").to_string_lossy().to_string();
    // log::info!("[get_suggested_output_path] 输出：{}", output_path);  // DISABLED
    Ok(output_path)
}

pub(crate) fn do_simplify_output_path(path: &std::path::Path) -> std::path::PathBuf {
    // Preserve the drive prefix (e.g., "D:") for Windows
    let mut result = std::path::PathBuf::new();

    // Check if path has a drive prefix
    let has_drive = path
        .components()
        .next()
        .map(|c| matches!(c, std::path::Component::Prefix(_)))
        .unwrap_or(false);

    if has_drive {
        // Get the drive prefix component
        if let Some(std::path::Component::Prefix(prefix)) = path.components().next() {
            result.push(prefix.as_os_str());
        }
    }

    // Get all components of the path (excluding the prefix)
    let components: Vec<_> = path
        .components()
        .skip(if has_drive { 1 } else { 0 })
        .filter_map(|c| match c {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    for name in components {
        // Rule 1: Remove directories starting with "c_"
        if name.starts_with("c_") {
            continue;
        }

        // Rule 2: Remove directories with names that are 3 or fewer digits only
        let is_short_numeric = name.chars().all(|c| c.is_ascii_digit()) && name.len() <= 3;
        if is_short_numeric {
            continue;
        }

        // Rule 3: Keep directories with names that are 5 or more digits only (keep as-is)
        // Rule 4: Keep all other directories
        result.push(&name);
    }

    // If result is empty or only has drive, use the original path
    let component_count = result.components().count();
    if component_count == 0 || (has_drive && component_count == 1) {
        return path.to_path_buf();
    }

    result
}

/// Simplify output path by removing unnecessary directory layers
/// Rules:
/// 1. Remove all directories starting with "c_"
/// 2. Remove directories with names that are 3 or fewer digits only
/// 3. Keep directories with names that are 5 or more digits only
/// 4. Keep all other directories
#[tauri::command]
fn simplify_output_path(path: &std::path::Path) -> std::path::PathBuf {
    do_simplify_output_path(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ========== 临时禁用日志系统以诊断问题 ==========
    // 如果程序能正常启动，说明问题在日志系统
    /*
    logger::init_logger(logger::LoggerConfig {
        log_dir: std::env::current_dir().unwrap().join("logs"),
        min_level: logger::LogLevel::Info,
        max_file_size: 10 * 1024 * 1024,
        max_files: 7,
        include_thread_id: true,
        include_location: true,
        sanitize_paths: true,
        log_format: logger::LogFormat::Plain,
        compress_old_logs: false,
    })
    .unwrap();
    */

    eprintln!("[警告] 日志系统已临时禁用用于诊断");
    // 立即设置 panic hook 以捕获任何早期崩溃
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unknown");

        eprintln!("===========================================");
        eprintln!("[PANIC] 程序发生严重错误！");
        eprintln!("线程: {}", thread_name);

        if let Some(location) = info.location() {
            eprintln!(
                "位置: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }

        if let Some(s) = info.payload().downcast_ref::<&str>() {
            eprintln!("信息: {}", s);
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            eprintln!("信息: {}", s);
        }

        eprintln!("堆栈跟踪:\n{}", backtrace);
        eprintln!("===========================================");

        // 尝试记录到日志（如果日志系统可用）- DISABLED
        let msg = format!("Panic in thread '{}': {:?}", thread_name, info);
        // log::error!("{}", msg);  // DISABLED: Logging temporarily disabled

        // 写入诊断文件
        if let Ok(mut file) =
            std::fs::File::create(std::env::temp_dir().join("bilibili-converter-panic.log"))
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", msg);
            let _ = writeln!(file, "{}", backtrace);
        }
    }));

    // 早期诊断输出，使用日志功能记录，同时打印到控制台
    eprintln!("[诊断] 程序启动 - 早期诊断检查");
    eprintln!("[诊断] 当前工作目录：{:?}", std::env::current_dir());
    eprintln!(
        "[诊断] 程序参数：{:?}",
        std::env::args().collect::<Vec<_>>()
    );

    // 写入诊断文件（简化版）
    let diagnostic_file = std::env::temp_dir().join("bilibili-converter-diagnostic.log");
    if let Ok(mut file) = std::fs::File::create(&diagnostic_file) {
        use std::io::Write;
        let _ = writeln!(file, "[诊断] 日志系统已禁用");
        let _ = writeln!(file, "[诊断] 如果程序能启动，说明问题在日志系统");
    }

    let app_state = Arc::new(AppState::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .manage(app_state)
        .setup(|app| {
            // DISABLED: 日志初始化已完全注释，仅使用 eprintln
            /*
            let app_handle = app.handle().clone();
            tokio::spawn(async move {
                // 等待一小段时间，让 GUI 先完成渲染
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                eprintln!("[日志] 开始在后台初始化日志系统...");

                if let Ok(log_dir) = app_handle.path().resource_dir() {
                    let log_path = log_dir.join("logs");

                    if std::fs::create_dir_all(&log_path).is_ok() {
                        let test_file = log_path.join(".write_test");
                        if std::fs::write(&test_file, "test").is_ok() {
                            let _ = std::fs::remove_file(test_file);

                            match logger::init_logger(logger::LoggerConfig {
                                log_dir: log_path,
                                min_level: logger::LogLevel::Info,
                                max_file_size: 10 * 1024 * 1024,
                                max_files: 3,
                                include_thread_id: false,
                                include_location: false,
                                sanitize_paths: true,
                                log_format: logger::LogFormat::Plain,
                                compress_old_logs: false,
                            }) {
                                Ok(_) => {
                                    eprintln!("[日志] ✅ 日志系统初始化成功（后台）");
                                    log::info!("[lib] 程序启动 - 日志系统已就绪（延迟初始化）");
                                }
                                Err(e) => {
                                    eprintln!("[日志] ❌ 初始化失败：{}", e);
                                }
                            }
                        } else {
                            eprintln!("[日志] ⚠️ 无写权限");
                        }
                    } else {
                        eprintln!("[日志] ⚠️ 无法创建日志目录");
                    }
                } else {
                    eprintln!("[日志] ⚠️ 无法获取资源目录");
                }
            });
            */

            eprintln!("[诊断] setup() 被调用 - 日志系统已禁用，仅使用 eprintln");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_folder,
            start_conversion,
            cancel_conversion,
            ensure_output_directory,
            get_ffmpeg_path,
            get_default_output_path, // For internal conversion (with simplification)
            get_suggested_output_path, // For UI display (without simplification)
            get_settings,
            open_output_folder,
            update_settings,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("[错误] Tauri 运行失败：{:?}", e);
            std::process::exit(1);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert!(settings.sound_enabled);
        assert_eq!(settings.output_format_video, "mp4");
        assert_eq!(settings.output_format_audio, "mp3");
        assert!(settings.output_path.is_empty());
    }

    #[test]
    fn test_app_settings_custom() {
        let settings = AppSettings {
            sound_enabled: false,
            output_format_video: "mkv".to_string(),
            output_format_audio: "flac".to_string(),
            output_path: "/custom/path".to_string(),
            concurrency: 8,
        };
        assert!(!settings.sound_enabled);
        assert_eq!(settings.output_format_video, "mkv");
        assert_eq!(settings.output_format_audio, "flac");
        assert_eq!(settings.output_path, "/custom/path");
        assert_eq!(settings.concurrency, 8);
    }

    #[test]
    fn test_media_file_structure() {
        let file = MediaFile {
            id: "test-id".to_string(),
            path: "/test/path.mp4".to_string(),
            name: "test.mp4".to_string(),
            size: 1024,
            file_type: "video".to_string(),
            title: "Test Video".to_string(),
            output_name: "output.mp4".to_string(),
            has_audio: Some(true),
        };
        assert_eq!(file.id, "test-id");
        assert_eq!(file.file_type, "video");
        assert_eq!(file.has_audio, Some(true));
    }

    #[test]
    fn test_conversion_progress() {
        let progress = ConversionProgress {
            file_id: "file-1".to_string(),
            file_name: "test.mp4".to_string(),
            progress: 50.0,
            status: "converting".to_string(),
            current_index: 1,
            completed_count: 0,
            total_count: 10,
            elapsed_time: 60,
            remaining_time: 60,
            conversion_speed: 10.5,
            average_speed: 9.8,
            estimated_size: 1048576,
            processed_bytes: 524288,
        };
        assert_eq!(progress.progress, 50.0);
        assert_eq!(progress.current_index, 1);
        assert_eq!(progress.conversion_speed, 10.5);
    }

    #[test]
    fn test_scan_result() {
        let files = vec![
            MediaFile {
                id: "1".to_string(),
                path: "/test/1.mp4".to_string(),
                name: "1.mp4".to_string(),
                size: 1024,
                file_type: "video".to_string(),
                title: "Video 1".to_string(),
                output_name: "1.mp4".to_string(),
                has_audio: Some(true),
            },
            MediaFile {
                id: "2".to_string(),
                path: "/test/2.mp3".to_string(),
                name: "2.mp3".to_string(),
                size: 512,
                file_type: "audio".to_string(),
                title: "Audio 1".to_string(),
                output_name: "2.mp3".to_string(),
                has_audio: None,
            },
        ];
        let result = ScanResult {
            files: files.clone(),
            total_size: 1536,
        };
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.total_size, 1536);
    }

    #[test]
    fn test_conversion_result() {
        let success_result = ConversionResult {
            file_id: "test".to_string(),
            success: true,
            output_path: Some("/output/test.mp4".to_string()),
            error: None,
        };
        assert!(success_result.success);
        assert!(success_result.output_path.is_some());

        let failure_result = ConversionResult {
            file_id: "test".to_string(),
            success: false,
            output_path: None,
            error: Some("FFmpeg error".to_string()),
        };
        assert!(!failure_result.success);
        assert!(failure_result.error.is_some());
    }
}
