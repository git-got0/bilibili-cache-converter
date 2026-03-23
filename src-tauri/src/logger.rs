//! Advanced logging system with file rotation and level filtering
//!
//! Features:
//! - Log level filtering (TRACE, DEBUG, INFO, WARN, ERROR)
//! - Daily log file rotation
//! - Size-based rotation (default 10MB per file)
//! - Real-time flushing for immediate persistence
//! - Thread-safe implementation
use chrono::Local;
use log::{LevelFilter, Log, Metadata, Record};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

/// Log levels in order of severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    #[default]
    Info = 2,
    Warn = 3,
    Error = 4,
}

/// 日志输出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LogFormat {
    /// 纯文本格式 (默认)
    #[default]
    Plain,
    /// JSON 格式，便于日志收集系统解析
    Json,
}

/// 日志条目结构 (用于 JSON 格式序列化)
#[derive(Debug, Clone, Serialize)]
struct LogEntryJson {
    timestamp: String,
    level: String,
    target: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(LogLevel::Trace),
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

/// Configuration for the logger
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Log directory path
    pub log_dir: PathBuf,
    /// Minimum log level to record
    pub min_level: LogLevel,
    /// Maximum size per log file in bytes (default 10MB)
    pub max_file_size: u64,
    /// Maximum number of log files to keep (0 = unlimited)
    pub max_files: usize,
    /// Include thread ID in logs
    pub include_thread_id: bool,
    /// Include source location (file:line) in logs
    pub include_location: bool,
    /// Enable sensitive path sanitization (replace user directories with ***)
    pub sanitize_paths: bool,
    /// Log output format (plain text or JSON)
    pub log_format: LogFormat,
    /// Enable log compression for old log files (gzip)
    pub compress_old_logs: bool,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("."),
            min_level: LogLevel::Info,
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 30,                   // Keep 30 days of logs
            include_thread_id: true,
            include_location: true,
            sanitize_paths: true,         // 默认启用路径脱敏
            log_format: LogFormat::Plain, // 默认纯文本格式
            compress_old_logs: false,     // 默认不压缩
        }
    }
}

/// Log entry metadata
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub thread_id: Option<u64>,
    pub location: Option<String>,
}

/// Global logger state
struct LoggerState {
    config: LoggerConfig,
    current_file: Option<BufWriter<File>>,
    current_file_date: String,
    current_file_size: AtomicU64,
    total_entries: AtomicU64,
    error_count: AtomicU64,
    /// 缓存的用户目录，用于路径脱敏
    user_home_dir: Option<PathBuf>,
}
static LOGGER: Lazy<Mutex<LoggerState>> = Lazy::new(|| {
    // 获取用户主目录用于路径脱敏
    let user_home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from);

    Mutex::new(LoggerState {
        config: LoggerConfig::default(),
        current_file: None,
        current_file_date: String::new(),
        current_file_size: AtomicU64::new(0),
        total_entries: AtomicU64::new(0),
        error_count: AtomicU64::new(0),
        user_home_dir,
    })
});

/// Global logger instance
static GLOBAL_LOGGER: SimpleLogger = SimpleLogger;
/// Wrapper for log::Log trait implementation
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Check if the log level is enabled based on our config
        // Default to INFO level if lock fails (allow most logs through)
        let min_level = LOGGER
            .lock()
            .map(|s| s.config.min_level)
            .unwrap_or(LogLevel::Info);

        let level = match metadata.level() {
            log::Level::Trace => LogLevel::Trace,
            log::Level::Debug => LogLevel::Debug,
            log::Level::Info => LogLevel::Info,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Error => LogLevel::Error,
        };

        level >= min_level
    }

    fn log(&self, record: &Record) {
        // Convert log::Level to our LogLevel
        let level = match record.level() {
            log::Level::Trace => LogLevel::Trace,
            log::Level::Debug => LogLevel::Debug,
            log::Level::Info => LogLevel::Info,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Error => LogLevel::Error,
        };

        // Use our internal log function
        log_internal(
            level,
            record.target(),
            &record.args().to_string(),
            Some(&format!(
                "{}:{}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0)
            )),
            None,
        );
    }

    fn flush(&self) {
        if let Ok(mut state) = LOGGER.lock() {
            if let Some(ref mut writer) = state.current_file {
                let _ = writer.flush();
            }
        }
    }
}

/// Initialize the logger with configuration
pub fn init_logger(config: LoggerConfig) -> Result<(), String> {
    // Ensure log directory exists
    if let Err(e) = fs::create_dir_all(&config.log_dir) {
        return Err(format!("Failed to create log directory: {}", e));
    }

    let mut state = LOGGER
        .lock()
        .map_err(|e| format!("Logger lock error: {}", e))?;
    state.config = config.clone();
    state.current_file = None;
    state.current_file_date = String::new();
    state.current_file_size.store(0, Ordering::Relaxed);

    // Initialize the log file directly (avoid deadlock by not calling log_internal)
    let today = get_date_string();
    let log_filename = format!("bilibili-converter-{}.log", today);
    let log_path = config.log_dir.join(&log_filename);

    match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(file) => {
            state.current_file = Some(BufWriter::new(file));
            state.current_file_date = today;
            // Write initialization message directly to avoid deadlock
            if let Some(ref mut writer) = state.current_file {
                let entry = format_log_entry(
                    &get_timestamp(),
                    LogLevel::Info,
                    "logger",
                    "Logger initialized successfully",
                    None,
                    None,
                );
                let entry_with_newline = format!("{}\n", entry);
                let _ = writer.write_all(entry_with_newline.as_bytes());
                let _ = writer.flush();
            }
        }
        Err(e) => {
            return Err(format!("Failed to create log file: {}", e));
        }
    }

    // Clean up old log files (outside the lock to avoid potential issues)
    drop(state);
    cleanup_old_logs()?;
    // ========== 新增：注册全局 logger ==========
    // 这使得 log::info! 等宏能够使用我们的自定义 logger
    let result = log::set_logger(&GLOBAL_LOGGER);
    match result {
        Ok(()) => {
            log::set_max_level(LevelFilter::Info);
            eprintln!("[Logger] Global logger registered successfully");
        }
        Err(e) => {
            eprintln!("[Logger] Failed to register global logger: {}", e);
        }
    }
    Ok(())
}

/// Update log directory (called when user changes output path)
pub fn update_log_directory(new_dir: PathBuf) -> Result<(), String> {
    // Ensure new directory exists
    if let Err(e) = fs::create_dir_all(&new_dir) {
        return Err(format!("Failed to create new log directory: {}", e));
    }

    let mut state = LOGGER
        .lock()
        .map_err(|e| format!("Logger lock error: {}", e))?;

    // Flush and close current file
    if let Some(ref mut writer) = state.current_file {
        let _ = writer.flush();
    }
    state.current_file = None;
    state.current_file_date = String::new();
    state.current_file_size.store(0, Ordering::Relaxed);

    // Update config
    state.config.log_dir = new_dir.clone();

    drop(state);

    // Log the change
    log_internal(
        LogLevel::Info,
        "logger",
        &format!("Log directory changed to: {}", new_dir.display()),
        None,
        None,
    );

    Ok(())
}

/// Set minimum log level
pub fn set_log_level(level: LogLevel) {
    if let Ok(mut state) = LOGGER.lock() {
        state.config.min_level = level;
    }
    log_internal(
        LogLevel::Info,
        "logger",
        &format!("Log level changed to: {}", level.as_str()),
        None,
        None,
    );
}

/// Get current log level
pub fn get_log_level() -> LogLevel {
    if let Ok(state) = LOGGER.lock() {
        state.config.min_level
    } else {
        LogLevel::Info
    }
}

/// Main logging function
pub fn log(level: LogLevel, target: &str, message: &str, location: Option<&str>) {
    // Fast check: skip if below minimum level
    let min_level = get_log_level();
    if level < min_level {
        return;
    }

    // Get thread ID
    let thread_id = std::thread::current().id();
    let thread_id_num = format!("{:?}", thread_id)
        .trim_start_matches("ThreadId(")
        .trim_end_matches(')')
        .parse::<u64>()
        .ok();

    log_internal(level, target, message, location, thread_id_num);
}

/// Internal logging implementation
fn log_internal(
    level: LogLevel,
    target: &str,
    message: &str,
    location: Option<&str>,
    thread_id: Option<u64>,
) {
    // Generate timestamp
    let timestamp = get_timestamp();

    // Format the log entry
    let entry = format_log_entry(&timestamp, level, target, message, location, thread_id);

    // Write to file
    if let Err(e) = write_to_file(&entry, level) {
        eprintln!("[LOGGER ERROR] Failed to write log: {}", e);
    }

    // Also output to console in debug mode
    #[cfg(debug_assertions)]
    {
        println!("{}", entry);
    }
}

/// Format a log entry (supports both plain text and JSON formats)
fn format_log_entry(
    timestamp: &str,
    level: LogLevel,
    target: &str,
    message: &str,
    location: Option<&str>,
    thread_id: Option<u64>,
) -> String {
    // 获取配置以确定日志格式
    let (log_format, sanitize_paths) = {
        if let Ok(state) = LOGGER.lock() {
            (state.config.log_format, state.config.sanitize_paths)
        } else {
            (LogFormat::Plain, true)
        }
    };

    match log_format {
        LogFormat::Json => format_log_entry_json(
            timestamp,
            level,
            target,
            message,
            location,
            thread_id,
            sanitize_paths,
        ),
        LogFormat::Plain => {
            // 纯文本格式：敏感路径脱敏
            let final_message = if sanitize_paths {
                sanitize_path(message)
            } else {
                message.to_string()
            };

            let mut entry = format!("[{}] [{:5}] [{}]", timestamp, level.as_str(), target);

            if let Some(tid) = thread_id {
                entry.push_str(&format!(" [T{}]", tid));
            }

            if let Some(loc) = location {
                entry.push_str(&format!(" [{}]", loc));
            }

            // 使用引用避免所有权问题
            entry.push_str(&format!(" {}", final_message.as_str()));

            entry
        }
    }
}

/// Write log entry to file with rotation
fn write_to_file(entry: &str, level: LogLevel) -> Result<(), String> {
    let mut state = LOGGER
        .lock()
        .map_err(|e| format!("Logger lock error: {}", e))?;

    // Check if we need a new file (date change or size limit)
    let today = get_date_string();
    let current_size = state.current_file_size.load(Ordering::Relaxed);

    let need_new_file = state.current_file.is_none()
        || state.current_file_date != today
        || current_size > state.config.max_file_size;

    if need_new_file {
        // Flush and close current file
        if let Some(ref mut writer) = state.current_file {
            let _ = writer.flush();
        }

        // Create new log file
        let log_filename = if state.current_file_date != today || state.current_file.is_none() {
            // New day: create dated file
            format!("bilibili-converter-{}.log", today)
        } else {
            // Size limit: add sequence number
            let seq = find_next_sequence(&state.config.log_dir, &today);
            format!("bilibili-converter-{}-{}.log", today, seq)
        };

        let log_path = state.config.log_dir.join(&log_filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file {:?}: {}", log_path, e))?;

        state.current_file = Some(BufWriter::new(file));
        state.current_file_date = today;
        state.current_file_size.store(0, Ordering::Relaxed);
    }

    // Write entry
    if let Some(ref mut writer) = state.current_file {
        let entry_with_newline = format!("{}\n", entry);
        let bytes = entry_with_newline.as_bytes();

        writer
            .write_all(bytes)
            .map_err(|e| format!("Failed to write log: {}", e))?;

        // Flush immediately for important logs
        if level >= LogLevel::Warn {
            let _ = writer.flush();
        }

        // Update size
        state
            .current_file_size
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
    }

    // Update counters
    state.total_entries.fetch_add(1, Ordering::Relaxed);
    if level == LogLevel::Error {
        state.error_count.fetch_add(1, Ordering::Relaxed);
    }

    Ok(())
}

/// Find next sequence number for log file
fn find_next_sequence(log_dir: &PathBuf, date: &str) -> usize {
    let mut max_seq = 1;

    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name();
            let name = filename.to_string_lossy();

            // Match pattern: bilibili-converter-YYYY-MM-DD-N.log
            if name.starts_with(&format!("bilibili-converter-{}", date)) {
                // Extract sequence number
                if let Some(pos) = name.rfind('-') {
                    if let Some(end) = name.find(".log") {
                        if let Ok(seq) = name[pos + 1..end].parse::<usize>() {
                            max_seq = max_seq.max(seq + 1);
                        }
                    }
                }
            }
        }
    }

    max_seq
}

/// Clean up old log files (with optional compression)
fn cleanup_old_logs() -> Result<(), String> {
    let (max_files, log_dir, compress_old_logs) = {
        let state = LOGGER
            .lock()
            .map_err(|e| format!("Logger lock error: {}", e))?;
        (
            state.config.max_files,
            state.config.log_dir.clone(),
            state.config.compress_old_logs,
        )
    };

    if max_files == 0 && !compress_old_logs {
        return Ok(()); // Unlimited files and no compression needed
    }

    let mut log_files: Vec<(String, std::time::SystemTime, bool)> = Vec::new();

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();

            // Match our log files (both .log and .log.gz)
            let is_gz = filename.ends_with(".log.gz");
            let is_plain_log =
                filename.starts_with("bilibili-converter-") && filename.ends_with(".log");

            if is_gz || is_plain_log {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        log_files.push((filename, modified, is_gz));
                    }
                }
            }
        }
    }

    // Sort by modification time (newest first)
    log_files.sort_by(|a, b| b.1.cmp(&a.1));

    // Separate plain log files from gzipped ones
    let plain_logs: Vec<_> = log_files.iter().filter(|(_, _, is_gz)| !is_gz).collect();
    let gzipped_logs: Vec<_> = log_files.iter().filter(|(_, _, is_gz)| *is_gz).collect();

    // Compress old plain log files if enabled
    if compress_old_logs {
        for (filename, modified, _) in plain_logs.iter() {
            let path = log_dir.join(filename);
            let age_days = SystemTime::now()
                .duration_since(*modified)
                .map(|d| d.as_secs() / 86400)
                .unwrap_or(0);

            // 压缩超过1天的日志文件
            if age_days >= 1 {
                let gz_path = log_dir.join(format!("{}.gz", filename));
                if !gz_path.exists() {
                    if let Err(e) = compress_log_file(&path, &gz_path) {
                        eprintln!("[LOGGER WARN] Failed to compress log {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    // Delete old files (count only plain logs + gzipped logs together for max_files)
    let total_files = plain_logs.len() + gzipped_logs.len();
    if total_files > max_files && max_files > 0 {
        // Keep the newest max_files
        for (filename, _, _is_gz) in log_files.iter().skip(max_files) {
            let path = log_dir.join(filename);
            if let Err(e) = fs::remove_file(&path) {
                eprintln!("[LOGGER WARN] Failed to delete old log {:?}: {}", path, e);
            }
        }
    }

    Ok(())
}

/// 使用 gzip 压缩单个日志文件
/// 注意: 此功能需要启用 flate2 特性 (在 Cargo.toml 中添加 flate2 依赖)
#[cfg(feature = "flate2")]
fn compress_log_file(source: &PathBuf, dest: &PathBuf) -> Result<(), String> {
    use std::io::Read;

    let mut file = File::open(source).map_err(|e| format!("Failed to open source file: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // 创建 GzEncoder，将数据写入 encoder，然后 finish 获取压缩后的数据
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&buffer).map_err(|e| format!("Failed to compress data: {}", e))?;
    let compressed_data = encoder
        .finish()
        .map_err(|e| format!("Failed to finish gzip encoding: {}", e))?;

    let mut dest_file =
        File::create(dest).map_err(|e| format!("Failed to create gz file: {}", e))?;
    dest_file
        .write_all(&compressed_data)
        .map_err(|e| format!("Failed to write gz file: {}", e))?;

    // 压缩成功后删除原文件
    fs::remove_file(source).map_err(|e| format!("Failed to delete original file: {}", e))?;

    Ok(())
}

/// 不使用压缩功能时的存根实现
#[cfg(not(feature = "flate2"))]
fn compress_log_file(_source: &PathBuf, _dest: &PathBuf) -> Result<(), String> {
    // 如果没有启用压缩功能，直接返回成功（不压缩）
    Ok(())
}

/// Flush all pending log entries
pub fn flush() {
    if let Ok(mut state) = LOGGER.lock() {
        if let Some(ref mut writer) = state.current_file {
            let _ = writer.flush();
        }
    }
}

/// Get logger statistics
pub fn get_stats() -> LoggerStats {
    if let Ok(state) = LOGGER.lock() {
        LoggerStats {
            total_entries: state.total_entries.load(Ordering::Relaxed),
            error_count: state.error_count.load(Ordering::Relaxed),
            current_file_size: state.current_file_size.load(Ordering::Relaxed),
            log_directory: state.config.log_dir.to_string_lossy().to_string(),
            min_level: state.config.min_level,
        }
    } else {
        LoggerStats::default()
    }
}

/// Logger statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoggerStats {
    pub total_entries: u64,
    pub error_count: u64,
    pub current_file_size: u64,
    pub log_directory: String,
    pub min_level: LogLevel,
}

/// Get current date string in YYYY-MM-DD format (for log filenames)
fn get_date_string() -> String {
    // Use chrono local time (东八区 for China)
    Local::now().format("%Y-%m-%d").to_string()
}

/// Get current timestamp in YYYY-MM-DD HH:MM:SS format (for log entries)
fn get_timestamp() -> String {
    // Use chrono local time (东八区 for China)
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 脱敏敏感路径信息，将用户目录替换为 ***
/// 例如: C:\Users\用户名\Documents\video.mp4 -> C:\Users\***\Documents\video.mp4
/// 同时也会处理常见的临时目录和下载目录
fn sanitize_path(path: &str) -> String {
    // 获取用户目录进行脱敏
    if let Ok(user_home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if path.starts_with(&user_home) {
            // 找到用户目录后的下一个路径分隔符位置
            if let Some(pos) = path[user_home.len()..].find(['/', '\\']) {
                let prefix = &path[..user_home.len() + pos + 1];
                let suffix = &path[user_home.len() + pos + 1..];
                return format!("{}***{}", prefix, suffix);
            }
        }
    }

    // 脱敏其他常见敏感路径
    let sensitive_patterns = [
        ("C:\\Users\\", "C:\\Users\\***\\"),
        ("/home/", "/home/***/"),
        ("/Users/", "/Users/***/"),
    ];

    let mut result = path.to_string();
    for (pattern, replacement) in sensitive_patterns {
        if result.starts_with(pattern) && !result.contains("***") {
            result = result.replacen(pattern, replacement, 1);
        }
    }

    result
}

/// 格式化日志条目为 JSON 格式
/// JSON 格式便于日志收集系统（如 ELK、 Loki）解析和分析
fn format_log_entry_json(
    timestamp: &str,
    level: LogLevel,
    target: &str,
    message: &str,
    location: Option<&str>,
    thread_id: Option<u64>,
    sanitize_paths: bool,
) -> String {
    // 如果启用路径脱敏，处理消息中的路径
    let sanitized_message = if sanitize_paths {
        sanitize_path(message)
    } else {
        message.to_string()
    };

    // 先尝试 JSON 序列化，失败后再使用纯文本
    let entry = LogEntryJson {
        timestamp: timestamp.to_string(),
        level: level.as_str().to_string(),
        target: target.to_string(),
        message: sanitized_message.clone(),
        thread_id,
        location: location.map(|s| s.to_string()),
    };

    // 使用 serde_json 序列化
    match serde_json::to_string(&entry) {
        Ok(json) => json,
        Err(_) => {
            // JSON 序列化失败时回退到纯文本
            format!(
                "[{}] [{:5}] [{}] {}",
                timestamp,
                level.as_str(),
                target,
                sanitized_message
            )
        }
    }
}

/// Check if a year is a leap year
// ============================================================================
// Convenience macros for logging
// ============================================================================

/// Log a trace message
#[macro_export]
macro_rules! log_trace {
    ($target:expr, $($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Trace,
            $target,
            &format!($($arg)*),
            Some(concat!(file!(), ":", line!()))
        )
    };
}

/// Log a debug message
#[macro_export]
macro_rules! log_debug {
    ($target:expr, $($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Debug,
            $target,
            &format!($($arg)*),
            Some(concat!(file!(), ":", line!()))
        )
    };
}

/// Log an info message
#[macro_export]
macro_rules! log_info {
    ($target:expr, $($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Info,
            $target,
            &format!($($arg)*),
            Some(concat!(file!(), ":", line!()))
        )
    };
}

/// Log a warning message
#[macro_export]
macro_rules! log_warn {
    ($target:expr, $($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Warn,
            $target,
            &format!($($arg)*),
            Some(concat!(file!(), ":", line!()))
        )
    };
}

/// Log an error message
#[macro_export]
macro_rules! log_error {
    ($target:expr, $($arg:tt)*) => {
        $crate::logger::log(
            $crate::logger::LogLevel::Error,
            $target,
            &format!($($arg)*),
            Some(concat!(file!(), ":", line!()))
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error > LogLevel::Warn);
        assert!(LogLevel::Warn > LogLevel::Info);
        assert!(LogLevel::Info > LogLevel::Debug);
        assert!(LogLevel::Debug > LogLevel::Trace);
    }

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("INFO"), Ok(LogLevel::Info));
        assert_eq!(LogLevel::from_str("error"), Ok(LogLevel::Error));
        assert!(LogLevel::from_str("invalid").is_err());
    }

    #[test]
    fn test_timestamp_format() {
        let ts = get_timestamp();
        assert!(ts.len() == 19); // YYYY-MM-DD HH:MM:SS
    }
}
