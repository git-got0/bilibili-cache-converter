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
            // Different file with same output name - use smarter naming strategy
            eprintln!(
                "[scanner] 检测到重复输出名：{}, 使用智能重命名...",
                output_name
            );

            // Get file extension
            let (base_name, ext_str) = if let Some(dot_pos) = output_name.rfind('.') {
                (&output_name[..dot_pos], &output_name[dot_pos..])
            } else {
                (output_name.as_str(), "")
            };

            // Strategy 1: Try to get part string from entry.json for unique naming
            let shortened = if let Some(entry_json_path) = find_entry_json(parent) {
                if let Ok(content) = std::fs::read_to_string(&entry_json_path) {
                    // Try to extract both title and part for better naming
                    let json_title_opt = extract_title_from_json(&content);
                    let part_opt = extract_part_from_json(&content);

                    match (json_title_opt, part_opt) {
                        (Some(t), Some(p)) => {
                            // Have both title and part: create unique name with title_P{part} format
                            let safe_title = sanitize_filename(&t);
                            format!(
                                "{}.{}",
                                truncate_title_with_part(&safe_title, &p.to_string(), ext_str),
                                ext_str
                            )
                        }
                        (Some(t), None) => {
                            // Only have title: use smart truncation
                            let safe_title = sanitize_filename(&t);
                            smart_truncate_middle(&safe_title, 60).to_string() + ext_str
                        }
                        (None, Some(p)) => {
                            // Only have part: add to base name
                            format!("{}_P{}{}", base_name, p, ext_str)
                        }
                        (None, None) => {
                            // No metadata available: use smart truncation with smaller limit
                            smart_truncate_middle(base_name, 50).to_string() + ext_str
                        }
                    }
                } else {
                    // Can't read entry.json: use smart truncation
                    smart_truncate_middle(base_name, 50).to_string() + ext_str
                }
            } else {
                // No entry.json found: use smart truncation
                smart_truncate_middle(base_name, 50).to_string() + ext_str
            };

            // Check if shortened name is available
            if !used_output_names.contains_key(&shortened) {
                output_name = shortened;
            } else {
                // Still duplicate, add counter suffix as last resort
                let mut counter = 2;
                loop {
                    let new_name = format!("{}_{}{}", base_name, counter, ext_str);
                    if !used_output_names.contains_key(&new_name) {
                        output_name = new_name;
                        break;
                    }
                    counter += 1;
                    if counter > 100 {
                        output_name = format!("{}_{}{}", base_name, counter, ext_str);
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
        // Try to extract part field - support both number and string types
        if let Some(part_value) = json.get("part") {
            // Try as number first (i64)
            if let Some(part_num) = part_value.as_i64() {
                return Some(part_num as i32);
            }
            // Try as string (parse to i32)
            if let Some(part_str) = part_value.as_str() {
                if let Ok(part_num) = part_str.parse::<i32>() {
                    return Some(part_num);
                }
            }
        }

        // Try nested paths (JSON pointer format: must start with /)
        for path in [
            "/entry/page_data/part",
            "/page_data/part",
            "/video_info/part",
            "/data/part",
            "/part",
        ] {
            if let Some(part_value) = json.pointer(path) {
                // Try as number first
                if let Some(part_num) = part_value.as_i64() {
                    return Some(part_num as i32);
                }
                // Try as string (parse to i32)
                if let Some(part_str) = part_value.as_str() {
                    if let Ok(part_num) = part_str.parse::<i32>() {
                        return Some(part_num);
                    }
                }
            }
        }
    }
    None
}

/// Extract part field as string (for better duplicate naming)
fn extract_part_string_from_json(content: &str) -> Option<String> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        // Try to extract part field as string
        if let Some(part) = json.get("part").and_then(|v| v.as_str()) {
            if !part.is_empty() {
                return Some(part.to_string());
            }
        }
        // Try nested paths (JSON pointer format: must start with /)
        for path in [
            "/entry/page_data/part",
            "/page_data/part",
            "/video_info/part",
            "/data/part",
            "/part",
        ] {
            if let Some(part) = json.pointer(path).and_then(|v| v.as_str()) {
                if !part.is_empty() {
                    return Some(part.to_string());
                }
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

            // Try to extract both title and part for better naming
            let json_title_opt = extract_title_from_json(&content);
            // Priority: string part first (e.g., "第一集"), then numeric part (e.g., 1)
            let part_string_opt = extract_part_string_from_json(&content);
            let part_number_opt = extract_part_from_json(&content);
            
            // Combine: prefer string part, fallback to numeric part
            let part_opt = part_string_opt.or_else(|| part_number_opt.map(|n| n.to_string()));

            // Priority 1: Use both title and part if available
            if let (Some(json_title), Some(part)) = (&json_title_opt, &part_opt) {
                eprintln!("[scanner] 使用标题 + 部分：{}_P{}", json_title, part);
                
                // Check if title and part are duplicates
                if is_title_part_duplicate(json_title, part) {
                    // Title and part are the same (after removing whitespace), use only part
                    eprintln!("[scanner] 检测到标题与部分重复，仅使用部分：{}", part);
                    let safe_part = sanitize_filename(part);
                    let truncated = smart_truncate_middle(&safe_part, 80);
                    return format!("{}.{}", truncated, ext);
                }
                
                let safe_json_title = sanitize_filename(json_title);
                return format!(
                    "{}.{}",
                    truncate_title_with_part(&safe_json_title, part, ext),
                    ext
                );
            }

            // Priority 2: Use only part if title not found
            if let Some(part) = &part_opt {
                eprintln!("[scanner] 仅使用部分：{}_P{}", safe_title, part);
                return format!("{}_P{}.{}", safe_title, part, ext);
            }

            // Priority 3: Use only title if part not found
            if let Some(json_title) = &json_title_opt {
                eprintln!("[scanner] 从 entry.json 使用标题：{}", json_title);
                let safe_json_title = sanitize_filename(json_title);
                let truncated = smart_truncate_middle(&safe_json_title, 80);
                return format!("{}.{}", truncated, ext);
            }

            eprintln!("[scanner] entry.json 中未找到部分或标题");
        } else {
            eprintln!("[scanner] 读取 entry.json 失败");
        }
    } else {
        eprintln!(
            "[scanner] 在 {:?} 或其父目录中未找到 entry.json",
            parent_dir
        );
    }

    // Fallback: use original title with smart truncation
    eprintln!("[scanner] 使用备用标题：{}", safe_title);
    let truncated_title = smart_truncate_middle(&safe_title, 80);
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

/// Truncate filename with "..." in the middle for long names
fn truncate_with_middle(base_name: &str, ext: &str) -> String {
    if base_name.len() > 30 {
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
            format!("{}{}", base_name, ext)
        }
    } else {
        format!("{}{}", base_name, ext)
    }
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

/// Smart truncate filename with "..." in the middle
/// Ensures that the reduced length is at least equal to the excess length
///
/// # Arguments
/// * `s` - Input string to truncate
/// * `max_len` - Maximum allowed length
///
/// # Returns
/// Truncated string with ellipsis in the middle if needed
fn smart_truncate_middle(s: &str, max_len: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let actual_len = chars.len();

    // No truncation needed
    if actual_len <= max_len {
        return s.to_string();
    }

    // Calculate how much we need to reduce
    let excess = actual_len - max_len;
    let ellipsis = "...";
    let ellipsis_len = ellipsis.chars().count();

    // We need to remove at least (excess + ellipsis_len) characters
    let min_remove = excess + ellipsis_len;

    // Calculate remaining length for the result
    let remaining_len = max_len - ellipsis_len;

    // Split evenly between start and end
    let mut start_len = remaining_len / 2;
    let mut end_len = remaining_len - start_len;

    // Adjust to ensure we're removing at least min_remove characters
    let total_removed = actual_len - (start_len + end_len + ellipsis_len);
    if total_removed < min_remove {
        // Need to remove more
        let additional_remove = min_remove - total_removed;
        // Reduce from both ends proportionally
        let reduce_start = additional_remove / 2;
        let reduce_end = additional_remove - reduce_start;
        start_len = start_len.saturating_sub(reduce_start);
        end_len = end_len.saturating_sub(reduce_end);
    }

    // Ensure we don't go negative
    start_len = start_len.max(0);
    end_len = end_len.max(0);

    // Build the result
    if start_len == 0 && end_len == 0 {
        // Edge case: string too short to keep any content
        return ellipsis.to_string();
    }

    let mut result = String::with_capacity(start_len + ellipsis_len + end_len);
    result.extend(chars.iter().take(start_len));
    result.push_str(ellipsis);
    if end_len > 0 {
        result.extend(chars.iter().skip(chars.len() - end_len));
    }

    result
}

/// Truncate title with part suffix, ensuring total length doesn't exceed reasonable limit
/// Format: title_P{part} or title..._P{part} if too long
///
/// # Arguments
/// * `title` - The title to truncate
/// * `part` - The part number suffix
/// * `ext` - File extension (for reference only, not included in return)
///
/// # Returns
/// Formatted string: "title_P{part}" (without extension)
fn truncate_title_with_part(title: &str, part: &str, _ext: &str) -> String {
    let base_max_len = 80; // Base maximum length for title
    let part_suffix = format!("_P{}", part);
    let full_format = format!("{}{}", title, part_suffix);

    if full_format.chars().count() <= base_max_len {
        // Doesn't exceed limit, use full format
        full_format
    } else {
        // Exceeds limit, need to truncate title
        // Priority: preserve part suffix completely, truncate title from end
        let part_suffix_len = part_suffix.chars().count();
        let ellipsis = "...";
        let ellipsis_len = ellipsis.chars().count();
        
        // Calculate available space for title (must include ellipsis)
        let title_available = base_max_len.saturating_sub(part_suffix_len + ellipsis_len);
        
        // Ensure minimum length for title (at least 10 chars to be meaningful)
        let title_max_len = title_available.max(10);
        
        // Truncate title from end only (preserve beginning, add "..." at end)
        let truncated_title = truncate_title_from_end(title, title_max_len);
        format!("{}{}{}", truncated_title, ellipsis, part_suffix)
    }
}

/// Truncate title from the end, preserving the beginning
/// This is used when part suffix needs to be preserved completely
///
/// # Arguments
/// * `title` - The title to truncate
/// * `max_len` - Maximum allowed length for title (without ellipsis)
///
/// # Returns
/// Truncated title (without ellipsis, caller should add it)
fn truncate_title_from_end(title: &str, max_len: usize) -> String {
    let chars: Vec<char> = title.chars().collect();
    
    if chars.len() <= max_len {
        return title.to_string();
    }
    
    // Take only the beginning part
    chars.iter().take(max_len).collect()
}

/// Check if title and part are duplicates (after removing whitespace)
/// Compares the first N characters where N is min(20, min(title_len, part_len))
///
/// # Arguments
/// * `title` - The title string
/// * `part` - The part string
///
/// # Returns
/// true if title and part are the same (after normalization)
fn is_title_part_duplicate(title: &str, part: &str) -> bool {
    // Remove all whitespace characters from both strings
    let title_normalized: String = title.chars().filter(|c| !c.is_whitespace()).collect();
    let part_normalized: String = part.chars().filter(|c| !c.is_whitespace()).collect();
    
    // If either is empty after normalization, not a duplicate
    if title_normalized.is_empty() || part_normalized.is_empty() {
        return false;
    }
    
    // Determine comparison length: min(20, min(title_len, part_len))
    let title_len = title_normalized.chars().count();
    let part_len = part_normalized.chars().count();
    let compare_len = 20.min(title_len.min(part_len));
    
    // Compare first N characters
    let title_prefix: String = title_normalized.chars().take(compare_len).collect();
    let part_prefix: String = part_normalized.chars().take(compare_len).collect();
    
    title_prefix == part_prefix
}
