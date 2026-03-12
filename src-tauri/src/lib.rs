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
    
    // Last resort: use current directory
    std::path::PathBuf::from("logs")
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

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, minutes, seconds)
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
    pub total_count: usize,
    pub elapsed_time: u64,
    pub remaining_time: u64,
    // Performance metrics
    pub conversion_speed: f64,    // MB/s
    pub average_speed: f64,        // Average MB/s
    pub estimated_size: u64,       // Estimated output size in bytes
    pub processed_bytes: u64,       // Bytes processed so far
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
    pub found_files: u32,
    pub current_path: String,
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
        Self {
            sound_enabled: true,
            output_format_video: "mp4".to_string(),
            output_format_audio: "mp3".to_string(),
            output_path: String::new(),
            concurrency: num_cpus::get(),
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
    pub pending_files: Mutex<Vec<MediaFile>>,  // Files pending to be processed after resume
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
    log::info!("Scanning folder: {}", folder_path);
    scanner::scan_bilibili_files(&folder_path, Some(app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_conversion(
    app: AppHandle,
    files: Vec<MediaFile>,
    folder_path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
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
async fn cancel_conversion(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<ConversionCancelledEvent, String> {
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
        log::info!("Killed all FFmpeg processes");
    }

    let mut tasks = state.conversion_tasks.lock().await;
    tasks.clear();

    let event = ConversionCancelledEvent {
        completed_count,
        total_count,
    };

    let _ = app.emit("conversion-cancelled", event.clone());

    log::info!("Conversion cancelled, completed: {}/{}", completed_count, total_count);
    Ok(event)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionPausedEvent {
    pub completed_count: usize,
    pub pending_count: usize,
}

#[tauri::command]
async fn pause_conversion(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<ConversionPausedEvent, String> {
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

    log::info!("Conversion paused, completed: {}/{}", completed_count, completed_count + pending_count);
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

    let _ = app.emit("conversion-resumed", ConversionResumedEvent {
        completed_count,
        pending_count,
    });

    log::info!("Conversion resumed, completed: {}, pending: {}", completed_count, pending_count);
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
    log::info!("Settings updated");
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

#[tauri::command]
async fn get_default_output_path(folder_path: String) -> Result<String, String> {
    use std::path::Path;
    
    // Validate input path
    let path = Path::new(&folder_path);
    if !path.is_absolute() {
        return Err("Invalid path: must be absolute path".to_string());
    }

    // Get parent directory and apply simplification rules
    let parent = path.parent().unwrap_or(path);
    let simplified = simplify_output_path(parent);
    
    let output_path = simplified
        .join("result")
        .to_string_lossy()
        .to_string();
    Ok(output_path)
}

/// Simplify output path by removing unnecessary directory layers
/// 
/// Rules:
/// 1. Remove all directories starting with "c_"
/// 2. Remove directories with names that are 3 or fewer digits only
/// 3. Keep directories with names that are 5 or more digits only
/// 4. Keep all other directories
fn simplify_output_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut result = std::path::PathBuf::new();
    
    // Get all components of the path
    let components: Vec<_> = path.components()
        .filter_map(|c| {
            match c {
                std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
                _ => None,
            }
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
    
    // If result is empty, use the original path
    if result.components().count() <= 1 {
        return path.to_path_buf();
    }
    
    result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .level_for("bilibili_converter", log::LevelFilter::Debug)
                .level_for("converter", log::LevelFilter::Debug)
                .level_for("scanner", log::LevelFilter::Debug)
                .format(|out, message, record| {
                    let timestamp = chrono_lite_timestamp();
                    let level = match record.level() {
                        log::Level::Error => "ERROR",
                        log::Level::Warn => "WARN",
                        log::Level::Info => "INFO",
                        log::Level::Debug => "DEBUG",
                        log::Level::Trace => "TRACE",
                    };
                    let target = record.target();
                    out.finish(format_args!("[{}] [{}] [{}] {}", timestamp, level, target, message))
                })
                .build(),
        )
        .manage(app_state)
        .setup(|app| {
            // ========== 第一部分：初始化日志系统 ==========
            // 优先使用程序安装目录（resource_dir），如果不可写则回退到 AppData
            let log_dir = determine_log_directory(app);
            
            // 初始化高级日志系统
            let logger_config = logger::LoggerConfig {
                log_dir: log_dir.clone(),
                min_level: logger::LogLevel::Info,
                max_file_size: 10 * 1024 * 1024, // 10MB
                max_files: 30,
                include_thread_id: true,
                include_location: true,
            };
            
            // 初始化日志系统
            if let Err(e) = logger::init_logger(logger_config) {
                eprintln!("Failed to initialize logger: {}", e);
            } else {
                logger::log(logger::LogLevel::Info, "startup", "========================================", None);
                logger::log(logger::LogLevel::Info, "startup", "应用启动 - 日志系统初始化成功", None);
                logger::log(logger::LogLevel::Info, "startup", &format!("日志目录: {}", log_dir.display()), None);
                logger::log(logger::LogLevel::Info, "startup", "========================================", None);
            }
            
            // ========== 第二部分：创建系统托盘图标 ==========
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Bilibili缓存转换器")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            log::info!("Application started successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_folder,
            start_conversion,
            pause_conversion,
            resume_conversion,
            cancel_conversion,
            get_settings,
            update_settings,
            open_output_folder,
            ensure_output_directory,
            get_ffmpeg_path,
            get_default_output_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
