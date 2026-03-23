use crate::{MediaFile, ScanProgress, ScanResult};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum ScanError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),
    #[error("Invalid path")]
    InvalidPath,
}

// Bilibili cache file patterns:
// - Video files: video.m4s (DASH), .blv (old format), .flv, .ts
// - Audio files: audio.m4s (DASH), .aac
// Note: .m4s files need to be checked by name (video.m4s vs audio.m4s) since both use .m4s extension
static VIDEO_EXTENSIONS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.(blv|flv|ts)$").unwrap());

static AUDIO_EXTENSIONS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.aac$").unwrap());

/// Determine the type of a Bilibili cache file
fn determine_file_type(_file_path: &Path, file_name: &str) -> Option<&'static str> {
    // Check specific .m4s files first (video.m4s vs audio.m4s)
    if file_name == "video.m4s" {
        return Some("video");
    }
    if file_name == "audio.m4s" {
        return Some("audio");
    }

    // Check other video formats by extension
    if VIDEO_EXTENSIONS.is_match(file_name) {
        return Some("video");
    }

    // Check other audio formats by extension
    if AUDIO_EXTENSIONS.is_match(file_name) {
        return Some("audio");
    }

    // No match
    None
}

#[allow(dead_code)]
static TITLE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+)\.blv$|video\.m4s$|audio\.m4s$").unwrap());

pub async fn scan_bilibili_files(
    folder_path: &str,
    app: Option<tauri::AppHandle>,
) -> Result<ScanResult, ScanError> {
    let path = Path::new(folder_path);

    // log::info!("开始扫描：{}", folder_path);  // DISABLED: Logging temporarily disabled

    // Validate path
    if !path.exists() {
        // log::error!("路径不存在：{}", folder_path);  // DISABLED
        return Err(ScanError::InvalidPath);
    }

    // Validate that path is a directory
    if !path.is_dir() {
        // log::error!("路径不是目录：{}", folder_path);  // DISABLED
        return Err(ScanError::InvalidPath);
    }

    // log::info!("路径验证通过，开始遍历目录树");  // DISABLED

    let mut files: Vec<MediaFile> = Vec::new();
    let mut total_size: u64 = 0;
    let mut id_counter: u64 = 0;
    let mut used_output_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Limit max depth to prevent deep recursion attacks
    const MAX_DEPTH: usize = 7;

    // 计数器用于诊断
    let mut entry_count: usize = 0;
    let mut error_count: usize = 0;

    for entry in WalkDir::new(path)
        .max_depth(MAX_DEPTH)
        .follow_links(false) // Don't follow symlinks to prevent infinite loops
        .into_iter()
        .filter_map(|e| match e.ok() {
            Some(entry) => {
                entry_count += 1;
                Some(entry)
            }
            None => {
                error_count += 1;
                None
            }
        })
    {
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }

        // 安全获取文件名
        let file_name = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => {
                eprintln!("[scanner] 警告：无法获取文件名，跳过：{:?}", file_path);
                continue;
            }
        };

        eprintln!(
            "[scanner] 找到媒体文件：{} (类型：{:?})",
            file_name,
            determine_file_type(file_path, file_name)
        );

        // Get parent directory info (安全版本)
        let parent = match file_path.parent() {
            Some(p) => p,
            None => {
                eprintln!(
                    "[scanner] 警告：无法获取父目录，使用文件路径本身：{:?}",
                    file_path
                );
                file_path
            }
        };

        // Try to extract title from directory structure
        let title = extract_title(parent, file_name);

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "[scanner] 警告：无法获取文件 {:?} 的元数据：{}. 跳过此文件。",
                    file_path, e
                );
                continue;
            }
        };
        let file_size = metadata.len();
        total_size += file_size;

        // safe unwrap: file_type was checked for None above, but add defensive check
        let file_type = match determine_file_type(file_path, file_name) {
            Some(t) => t,
            None => {
                eprintln!("[scanner] 错误：检查后仍然无法确定文件类型：{}", file_name);
                continue;
            }
        };

        // Skip audio files that are part of video (they will be combined)
        // Only skip if this is audio.m4s and video.m4s exists in same directory
        if file_type == "audio" && parent.join("video.m4s").exists() {
            eprintln!("[scanner] 跳过音频文件（有视频）：{}", file_name);
            continue;
        }

        id_counter += 1;
        let id = format!("file_{}", id_counter);

        // Check if audio.m4s exists in the same directory for video files
        let has_audio = if file_type == "video" {
            parent.join("audio.m4s").exists()
        } else {
            false
        };

        // Generate output name with part
        let mut output_name = generate_output_name_with_part(&title, file_type, parent);

        // Handle duplicate output names
        if let Some(existing_path) = used_output_names.get(&output_name) {
            // Check if it's the same source file
            if existing_path == &file_path.to_string_lossy().to_string() {
                // Same file, skip
                eprintln!("[scanner] 跳过重复文件：{}", file_name);
                continue;
            }
            // Different file with same output name - try to shorten and add suffix
            eprintln!("[scanner] 检测到重复输出名：{}, 正在缩短...", output_name);

            // Get file extension
            let (base_name, ext) = if let Some(dot_pos) = output_name.rfind('.') {
                (&output_name[..dot_pos], &output_name[dot_pos..])
            } else {
                (output_name.as_str(), "")
            };

            // Try shortening middle to "..."
            let shortened = if base_name.len() > 30 {
                // 使用 chars() 而不是字节索引，避免切分 UTF-8 字符
                let chars: Vec<char> = base_name.chars().collect();
                if chars.len() > 30 {
                    let start_len = 15.min(chars.len());
                    let end_start = (chars.len() - 15).max(start_len);
                    let new_base: String = chars[..start_len]
                        .iter()
                        .chain(['.', '.', '.'].iter())
                        .chain(chars[end_start..].iter())
                        .collect();
                    format!("{}{}", new_base, ext)
                } else {
                    output_name.clone()
                }
            } else {
                output_name.clone()
            };

            // Check if shortened name is available
            if !used_output_names.contains_key(&shortened) {
                output_name = shortened;
            } else {
                // Still duplicate, add suffix
                let mut counter = 2;
                loop {
                    let new_name = format!("{}_{}{}", base_name, counter, ext);
                    if !used_output_names.contains_key(&new_name) {
                        output_name = new_name;
                        break;
                    }
                    counter += 1;
                    if counter > 100 {
                        // Fallback to original with counter
                        output_name = format!("{}_{}{}", base_name, counter, ext);
                        break;
                    }
                }
            }
        }

        // Record this output name with its source path
        used_output_names.insert(output_name.clone(), file_path.to_string_lossy().to_string());

        let media_file = MediaFile {
            id,
            path: file_path.to_string_lossy().to_string(),
            name: file_name.to_string(),
            size: file_size,
            file_type: file_type.to_string(),
            title,
            output_name,
            has_audio: Some(has_audio),
        };

        // Emit scan progress event for real-time update
        if let Some(ref app_handle) = app {
            let _ = app_handle.emit(
                "scan-progress",
                ScanProgress {
                    processed: files.len() as u64,
                    total: 0, // Unknown total
                    message: format!("已找到 {} 个文件...", files.len()),
                },
            );
        }

        files.push(media_file);
    }

    // Remove duplicates and organize files
    // For video files with audio, keep only video (audio will be combined)
    // For standalone audio files, keep them as well
    let mut unique_files: Vec<MediaFile> = Vec::new();
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    eprintln!("[scanner] 处理 {} 个原始文件（去重前）", files.len());

    for file in files {
        if file.file_type == "video" {
            // Keep video files (skip if already seen)
            if !seen_paths.contains(&file.path) {
                seen_paths.insert(file.path.clone());
                eprintln!("[scanner] 添加视频文件：{}", file.name);
                unique_files.push(file);
            }
        } else if file.file_type == "audio" {
            // Keep standalone audio files (not part of a video)
            // These are audio files without a corresponding video.m4s
            let parent = Path::new(&file.path)
                .parent()
                .unwrap_or(Path::new(&file.path));
            let has_video = parent.join("video.m4s").exists();

            if !has_video && !seen_paths.contains(&file.path) {
                seen_paths.insert(file.path.clone());
                eprintln!("[scanner] 添加独立音频文件：{}", file.name);
                unique_files.push(file);
            } else if has_video {
                eprintln!("[scanner] 跳过音频文件（有视频）：{}", file.name);
            }
        }
    }

    eprintln!(
        "[scanner] 最终结果：{} 个唯一文件 (总大小：{:.2} MB)",
        unique_files.len(),
        total_size as f64 / (1024.0 * 1024.0)
    );

    for (idx, file) in unique_files.iter().enumerate() {
        eprintln!(
            "[scanner] 文件 {}: {} ({}) - {}",
            idx + 1,
            file.name,
            file.file_type,
            file.title
        );
    }

    Ok(ScanResult {
        files: unique_files,
        total_size,
    })
}

fn extract_title(parent: &Path, file_name: &str) -> String {
    // Try to find title from directory structure
    // Common patterns: .../P123456/ or .../title/

    // Check for danmaku or entry files that might contain title info
    let potential_title_files = ["entry.json", "info.json", "title.txt"];

    for entry in WalkDir::new(parent)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let entry_name = entry
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if potential_title_files.contains(&entry_name) {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if let Some(title) = extract_title_from_json(&content) {
                    return title;
                }
            }
        }
    }

    // Try to extract from folder name
    let folder_name = parent
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    if folder_name.len() > 3 {
        return folder_name.to_string();
    }

    // Use filename without extension
    Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未知标题")
        .to_string()
}

fn extract_title_from_json(content: &str) -> Option<String> {
    // Try to parse as JSON and extract title
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        // Try common title fields
        let title_paths = [
            "title",
            "Title",
            "page_data/title",
            "video_info/title",
            "data/title",
        ];

        for path in title_paths {
            // Use pointer() for nested paths
            let value = if path.contains('/') {
                json.pointer(path)
            } else {
                json.get(path)
            };

            if let Some(title) = value.and_then(|v| v.as_str()) {
                if !title.is_empty() {
                    eprintln!("[scanner] 在路径 {} 找到标题：{}", path, title);
                    return Some(truncate_chinese(title, 80));
                }
            }
        }
    }
    eprintln!("[scanner] JSON 中未找到标题");
    None
}

fn extract_part_from_json(content: &str) -> Option<i32> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        // Try to extract part field (video part number)
        if let Some(part) = json.get("part").and_then(|v| v.as_i64()) {
            return Some(part as i32);
        }
        // Try nested paths
        for path in ["page_data.part", "video_info.part", "data.part"] {
            if let Some(part) = json.pointer(path).and_then(|v| v.as_i64()) {
                return Some(part as i32);
            }
        }
    }
    None
}

fn generate_output_name_with_part(title: &str, file_type: &str, parent_dir: &Path) -> String {
    let safe_title = sanitize_filename(title);
    let ext = if file_type == "video" { "mp4" } else { "mp3" };

    // Try to find entry.json in parent directory (one level up from media files)
    if let Some(entry_json_path) = find_entry_json(parent_dir) {
        eprintln!("[scanner] 在 {:?} 找到 entry.json", entry_json_path);
        if let Ok(content) = std::fs::read_to_string(&entry_json_path) {
            eprintln!(
                "[scanner] entry.json 内容：{}",
                &content[..content.len().min(500)]
            );
            // Try to use part first
            if let Some(part) = extract_part_from_json(&content) {
                eprintln!("[scanner] 使用部分：{}", part);
                return format!("{}_P{}.{}", safe_title, part, ext);
            }
            // Fallback to title if part not found
            if let Some(json_title) = extract_title_from_json(&content) {
                eprintln!("[scanner] 从 entry.json 使用标题：{}", json_title);
                let safe_json_title = sanitize_filename(&json_title);
                let truncated_title = truncate_chinese(&safe_json_title, 80);
                return format!("{}.{}", truncated_title, ext);
            }
            eprintln!("[scanner] entry.json 中未找到部分或标题");
        }
    } else {
        eprintln!(
            "[scanner] 在 {:?} 或其父目录中未找到 entry.json",
            parent_dir
        );
    }
    // Fallback to original title naming
    eprintln!("[scanner] 使用备用标题：{}", title);
    let truncated_title = truncate_chinese(&safe_title, 80);
    format!("{}.{}", truncated_title, ext)
}

fn find_entry_json(parent_dir: &Path) -> Option<PathBuf> {
    // Check current directory
    let entry_path = parent_dir.join("entry.json");
    if entry_path.exists() {
        return Some(entry_path);
    }
    // Check parent directory (one level up)
    if let Some(grandparent) = parent_dir.parent() {
        let entry_path = grandparent.join("entry.json");
        if entry_path.exists() {
            return Some(entry_path);
        }
    }
    None
}

fn sanitize_filename(name: &str) -> String {
    // More comprehensive sanitization to prevent path traversal and injection
    let invalid_chars = [
        '<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0', '\n', '\r', '\t',
    ];
    let mut result = String::new();

    for c in name.chars() {
        if invalid_chars.contains(&c) {
            result.push('_');
        } else {
            // Replace sequences of dots to prevent path traversal
            if c == '.' {
                // Check if the last character is already a dot
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

    // Trim and ensure not empty
    let trimmed = result.trim();
    if trimmed.is_empty() {
        "output".to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate_chinese(s: &str, max_len: usize) -> String {
    let mut count = 0;
    let mut result = String::new();

    for c in s.chars() {
        if count >= max_len {
            break;
        }
        // Chinese characters count as 2
        count += if c.is_ascii() { 1 } else { 2 };
        result.push(c);
    }

    if result.len() < s.len() {
        result.push_str("...");
    }

    result
}
