//! Advanced logging system with file rotation and level filtering
//!
//! Features:
//! - Log level filtering (TRACE, DEBUG, INFO, WARN, ERROR)
//! - Daily log file rotation
//! - Size-based rotation (default 10MB per file)
//! - Real-time flushing for immediate persistence
//! - Thread-safe implementation

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
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("."),
            min_level: LogLevel::Info,
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 30,                    // Keep 30 days of logs
            include_thread_id: true,
            include_location: true,
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
}

/// Global logger instance
static LOGGER: Lazy<Mutex<LoggerState>> = Lazy::new(|| {
    Mutex::new(LoggerState {
        config: LoggerConfig::default(),
        current_file: None,
        current_file_date: String::new(),
        current_file_size: AtomicU64::new(0),
        total_entries: AtomicU64::new(0),
        error_count: AtomicU64::new(0),
    })
});

/// Initialize the logger with configuration
pub fn init_logger(config: LoggerConfig) -> Result<(), String> {
    // Ensure log directory exists
    if let Err(e) = fs::create_dir_all(&config.log_dir) {
        return Err(format!("Failed to create log directory: {}", e));
    }

    let mut state = LOGGER.lock().map_err(|e| format!("Logger lock error: {}", e))?;
    state.config = config.clone();
    state.current_file = None;
    state.current_file_date = String::new();
    state.current_file_size.store(0, Ordering::Relaxed);

    // Initialize the log file directly (avoid deadlock by not calling log_internal)
    let today = get_date_string();
    let log_filename = format!("bilibili-converter-{}.log", today);
    let log_path = config.log_dir.join(&log_filename);

    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
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

    Ok(())
}

/// Update log directory (called when user changes output path)
pub fn update_log_directory(new_dir: PathBuf) -> Result<(), String> {
    // Ensure new directory exists
    if let Err(e) = fs::create_dir_all(&new_dir) {
        return Err(format!("Failed to create new log directory: {}", e));
    }

    let mut state = LOGGER.lock().map_err(|e| format!("Logger lock error: {}", e))?;

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
    log_internal(LogLevel::Info, "logger", &format!("Log directory changed to: {}", new_dir.display()), None, None);

    Ok(())
}

/// Set minimum log level
pub fn set_log_level(level: LogLevel) {
    if let Ok(mut state) = LOGGER.lock() {
        state.config.min_level = level;
    }
    log_internal(LogLevel::Info, "logger", &format!("Log level changed to: {}", level.as_str()), None, None);
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
pub fn log(
    level: LogLevel,
    target: &str,
    message: &str,
    location: Option<&str>,
) {
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

/// Format a log entry
fn format_log_entry(
    timestamp: &str,
    level: LogLevel,
    target: &str,
    message: &str,
    location: Option<&str>,
    thread_id: Option<u64>,
) -> String {
    let mut entry = format!("[{}] [{:5}] [{}]", timestamp, level.as_str(), target);

    if let Some(tid) = thread_id {
        entry.push_str(&format!(" [T{}]", tid));
    }

    if let Some(loc) = location {
        entry.push_str(&format!(" [{}]", loc));
    }

    entry.push_str(&format!(" {}", message));

    entry
}

/// Write log entry to file with rotation
fn write_to_file(entry: &str, level: LogLevel) -> Result<(), String> {
    let mut state = LOGGER.lock().map_err(|e| format!("Logger lock error: {}", e))?;

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
        state.current_file_size.fetch_add(bytes.len() as u64, Ordering::Relaxed);
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

/// Clean up old log files
fn cleanup_old_logs() -> Result<(), String> {
    let state = LOGGER.lock().map_err(|e| format!("Logger lock error: {}", e))?;
    let max_files = state.config.max_files;
    let log_dir = state.config.log_dir.clone();
    drop(state);

    if max_files == 0 {
        return Ok(()); // Unlimited files
    }

    let mut log_files: Vec<(String, std::time::SystemTime)> = Vec::new();

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();

            // Only match our log files
            if filename.starts_with("bilibili-converter-") && filename.ends_with(".log") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        log_files.push((filename, modified));
                    }
                }
            }
        }
    }

    // Sort by modification time (newest first)
    log_files.sort_by(|a, b| b.1.cmp(&a.1));

    // Delete old files
    if log_files.len() > max_files {
        for (filename, _) in log_files.iter().skip(max_files) {
            let path = log_dir.join(filename);
            if let Err(e) = fs::remove_file(&path) {
                eprintln!("[LOGGER WARN] Failed to delete old log {:?}: {}", path, e);
            }
        }
    }

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
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;

    // Calculate year, month, day
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

    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Get current timestamp in YYYY-MM-DD HH:MM:SS format (for log entries)
fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Calculate year, month, day
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

/// Check if a year is a leap year
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

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
