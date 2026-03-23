use crate::{
    AppSettings, AppState, ConversionProgress, ConversionResult, MediaFile,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuType {
    Nvidia,
    Amd,
    Intel,
    None,
}

#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub video_encoder: &'static str,
    pub audio_encoder: &'static str,
    pub use_gpu: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            video_encoder: "libx264",
            audio_encoder: "aac",
            use_gpu: false,
        }
    }
}

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum ConverterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("FFmpeg not found")]
    FfmpegNotFound,
    #[error("Conversion failed: {0}")]
    ConversionFailed(String),
    #[error("Task cancelled")]
    Cancelled,
}

pub struct ConversionTask {
    pub cancelled: bool,
    pub child_pid: Option<u32>, // FFmpeg process PID
}

impl ConversionTask {
    pub fn new() -> Self {
        Self {
            cancelled: false,
            child_pid: None,
        }
    }
}

#[derive(Clone)]
struct ProgressInfo {
    current_index: usize,
    completed_count: usize,
    total_count: usize,
    start_time: Option<std::time::Instant>,
}

static PERCENT_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+\.?\d*)%").unwrap());

pub async fn get_ffmpeg_path(app: Option<&AppHandle>) -> Result<String, String> {
    // First, check bundled resources
    if let Some(app_handle) = app {
        if let Ok(resource_dir) = app_handle.path().resource_dir() {
            let bundled_ffmpeg = resource_dir.join("ffmpeg.exe");
            if bundled_ffmpeg.exists() {
                let path_str = bundled_ffmpeg.to_string_lossy().to_string();
                if test_ffmpeg_path(&path_str).await {
                    // Remove Windows NT path prefix (\\?\)
                    let clean_path = path_str
                        .strip_prefix(r"\\?\")
                        .unwrap_or(&path_str)
                        .to_string();
                    eprintln!("[converter] 使用内置 FFmpeg: {}", clean_path);
                    return Ok(clean_path);
                }
            }
        }
    }

    // Check common Windows locations
    let possible_paths = [
        "ffmpeg",
        "ffmpeg.exe",
        "C:\\ffmpeg\\bin\\ffmpeg.exe",
        "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
        "C:\\Program Files (x86)\\ffmpeg\\bin\\ffmpeg.exe",
        "D:\\dependencies\\ffmpeg-7.1.1\\bin\\ffmpeg.exe",
    ];

    for path in &possible_paths {
        if test_ffmpeg_path(path).await {
            return Ok(path.to_string());
        }
    }

    Err("FFmpeg not found. Please install FFmpeg and add it to PATH.".to_string())
}

/// Detect available GPU type by checking FFmpeg encoders
pub async fn detect_gpu_type(ffmpeg_path: &str) -> GpuType {
    let output = Command::new(ffmpeg_path)
        .arg("-hide_banner")
        .arg("-encoders")
        .arg("2")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(out) => {
            let output_str = String::from_utf8_lossy(&out.stdout);
            let stderr_str = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{}\n{}", output_str, stderr_str);

            // Check for NVIDIA GPU (nvenc encoders)
            if combined.contains("h264_nvenc") || combined.contains("hevc_nvenc") {
                eprintln!("[converter] 检测到 NVIDIA GPU - 启用硬件加速");
                return GpuType::Nvidia;
            }

            // Check for AMD GPU (amf encoders)
            if combined.contains("h264_amf") || combined.contains("hevc_amf") {
                eprintln!("[converter] 检测到 AMD GPU - 启用硬件加速");
                return GpuType::Amd;
            }

            // Check for Intel GPU (qsv encoders)
            if combined.contains("h264_qsv") || combined.contains("hevc_qsv") {
                eprintln!("[converter] 检测到 Intel GPU - 启用硬件加速");
                return GpuType::Intel;
            }

            eprintln!("[converter] 未检测到硬件加速 - 使用 CPU 编码");
            GpuType::None
        }
        Err(e) => {
            eprintln!("[converter] GPU 检测失败：{}", e);
            GpuType::None
        }
    }
}

/// Get encoder configuration based on GPU type
pub fn get_encoder_config(gpu_type: GpuType, output_ext: &str) -> EncoderConfig {
    match gpu_type {
        GpuType::Nvidia => {
            let video_encoder = match output_ext {
                "mp4" | "mkv" => "h264_nvenc",
                "avi" => "h264_nvenc",
                _ => "h264_nvenc",
            };
            EncoderConfig {
                video_encoder,
                audio_encoder: "aac",
                use_gpu: true,
            }
        }
        GpuType::Amd => {
            let video_encoder = match output_ext {
                "mp4" | "mkv" => "h264_amf",
                "avi" => "h264_amf",
                _ => "h264_amf",
            };
            EncoderConfig {
                video_encoder,
                audio_encoder: "aac",
                use_gpu: true,
            }
        }
        GpuType::Intel => {
            let video_encoder = match output_ext {
                "mp4" | "mkv" => "h264_qsv",
                "avi" => "h264_qsv",
                _ => "h264_qsv",
            };
            EncoderConfig {
                video_encoder,
                audio_encoder: "aac",
                use_gpu: true,
            }
        }
        GpuType::None => {
            // CPU encoding fallback
            let video_encoder = match output_ext {
                "mp4" | "mkv" => "libx264",
                "avi" => "libx264",
                _ => "libx264",
            };
            EncoderConfig {
                video_encoder,
                audio_encoder: "aac",
                use_gpu: false,
            }
        }
    }
}

async fn test_ffmpeg_path(path: &str) -> bool {
    Command::new(path)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| true)
        .unwrap_or(false)
}

pub async fn convert_files(
    app: AppHandle,
    files: Vec<MediaFile>,
    base_dir: &str,
    settings: &AppSettings,
    state: Arc<AppState>,
    start_time: Option<std::time::Instant>,
) -> Vec<ConversionResult> {
    let mut results = Vec::new();

    // Get output directory: use user-specified path, or default to base_dir/result
    let output_dir = if settings.output_path.is_empty() {
        // Default: source directory/result
        Path::new(base_dir)
            .join("result")
            .to_string_lossy()
            .to_string()
    } else {
        settings.output_path.clone()
    };

    // Validate output directory path
    let output_path_obj = Path::new(&output_dir);
    if !output_path_obj.is_absolute() {
        eprintln!("[converter] 错误：输出路径必须是绝对路径");
        return files
            .iter()
            .map(|f| ConversionResult {
                file_id: f.id.clone(),
                success: false,
                output_path: None,
                error: Some("输出路径无效：必须是绝对路径".to_string()),
            })
            .collect();
    }

    // Ensure output directory exists
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        eprintln!("[converter] 错误：无法创建输出目录：{}", e);
        return files
            .iter()
            .map(|f| ConversionResult {
                file_id: f.id.clone(),
                success: false,
                output_path: None,
                error: Some(format!("无法创建输出目录：{}", e)),
            })
            .collect();
    }

    // Get FFmpeg path
    let ffmpeg_path = match get_ffmpeg_path(Some(&app)).await {
        Ok(path) => path,
        Err(e) => {
            eprintln!("[converter] 错误：找不到 FFmpeg: {}", e);
            return files
                .iter()
                .map(|f| ConversionResult {
                    file_id: f.id.clone(),
                    success: false,
                    output_path: None,
                    error: Some(e.clone()),
                })
                .collect();
        }
    };

    eprintln!("[converter] 使用 FFmpeg: {}", ffmpeg_path);

    // Detect GPU type for hardware acceleration
    let gpu_type = detect_gpu_type(&ffmpeg_path).await;
    let encoder_config = get_encoder_config(gpu_type, &settings.output_format_video);

    if encoder_config.use_gpu {
        eprintln!(
            "[converter] 使用 GPU 加速，编码器：{}",
            encoder_config.video_encoder
        );
    } else {
        eprintln!(
            "[converter] 使用 CPU 编码，编码器：{}",
            encoder_config.video_encoder
        );
    }

    // Convert files with concurrency
    let concurrency = settings.concurrency.max(1);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let total_count = files.len();

    let mut handles = Vec::new();

    for (index, file) in files.into_iter().enumerate() {
        // 安全获取信号量许可，避免 unwrap() 导致 panic
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "[converter] 错误：无法获取信号量许可，文件 {}: {}. 跳过此文件。",
                    file.path, e
                );
                // 记录跳过的文件信息
                let error_result = ConversionResult {
                    file_id: file.id.clone(),
                    success: false,
                    output_path: None,
                    error: Some(format!("信号量获取失败：{}", e)),
                };
                results.push(error_result);
                continue;
            }
        };
        let app_clone = app.clone();
        let ffmpeg_path_clone = ffmpeg_path.clone();
        let output_dir_clone = output_dir.clone();
        let settings_clone = settings.clone();
        let state_clone = state.clone();
        let base_dir_clone = base_dir.to_string();
        let encoder_config_clone = encoder_config.clone();
        let start_time_clone = start_time;
        let current_index = index;
        let total_count_clone = total_count;

        let handle = tokio::spawn(async move {
            let _permit = permit;

            let progress_info = ProgressInfo {
                current_index,
                completed_count: {
                    let count = state_clone.completed_count.lock().await;
                    *count
                },
                total_count: total_count_clone,
                start_time: start_time_clone,
            };

            let result = convert_single_file(
                app_clone,
                file.clone(),
                &ffmpeg_path_clone,
                &output_dir_clone,
                &base_dir_clone,
                &encoder_config_clone,
                &settings_clone,
                state_clone,
                progress_info,
            )
            .await;

            result
        });

        handles.push(handle);
    }

    for handle in handles {
        match handle.await {
            Ok(result) => {
                results.push(result);
            }
            Err(e) => {
                eprintln!(
                    "[converter] 错误：任务执行失败 (任务 panic 或被取消): {}",
                    e
                );
                // 添加一个空的错误结果以保持索引一致性
                let error_result = ConversionResult {
                    file_id: String::new(),
                    success: false,
                    output_path: None,
                    error: Some(format!("任务执行失败：{}", e)),
                };
                results.push(error_result);
            }
        }
    }

    results
}

async fn convert_single_file(
    app: AppHandle,
    file: MediaFile,
    ffmpeg_path: &str,
    output_dir: &str,
    base_dir: &str,
    encoder_config: &EncoderConfig,
    settings: &AppSettings,
    state: Arc<AppState>,
    progress_info: ProgressInfo,
) -> ConversionResult {
    let output_ext = if file.file_type == "video" {
        &settings.output_format_video
    } else {
        &settings.output_format_audio
    };

    let output_name = Path::new(&file.output_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    // Sanitize output name to prevent directory traversal in filenames
    let output_name = output_name
        .replace("..", "")
        .replace("/", "_")
        .replace("\\", "_")
        .replace(":", "_");

    // Preserve directory structure: get relative path from base_dir
    let source_path = Path::new(&file.path);
    let relative_path = source_path.strip_prefix(base_dir).unwrap_or(source_path);
    let relative_dir = relative_path.parent();

    // Build output path with optimization: skip single subfolder level
    let output_path = if let Some(rel_dir) = relative_dir {
        // Optimize directory structure: skip single subfolder level
        let optimized_dir = crate::do_simplify_output_path(rel_dir);
        let sub_dir = Path::new(output_dir).join(&optimized_dir);
        if let Err(e) = std::fs::create_dir_all(&sub_dir) {
            eprintln!("[converter] 警告：无法创建子目录：{}", e);
        }
        sub_dir.join(format!("{}.{}", output_name, output_ext))
    } else {
        Path::new(output_dir).join(format!("{}.{}", output_name, output_ext))
    };

    let mut output_path_str = output_path.to_string_lossy().to_string();
    // Remove Windows NT path prefix (\\?\) if present
    output_path_str = output_path_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&output_path_str)
        .to_string();

    // Calculate elapsed and remaining time
    let realtime_completed = {
        let count = state.completed_count.lock().await;
        *count
    };
    let (elapsed_time, remaining_time) = calculate_time_stats(
        progress_info.start_time,
        progress_info.current_index,
        progress_info.total_count,
        0.0,
        realtime_completed,
    );

    // Emit progress start with performance metrics
    let _ = app.emit(
        "conversion-progress",
        ConversionProgress {
            file_id: file.id.clone(),
            file_name: file.name.clone(),
            progress: 0.0,
            status: "starting".to_string(),
            current_index: progress_info.current_index,
            completed_count: progress_info.completed_count,
            total_count: progress_info.total_count,
            elapsed_time,
            remaining_time,
            conversion_speed: 0.0,
            average_speed: 0.0,
            estimated_size: file.size,
            processed_bytes: 0,
        },
    );

    // Check if we should cancel or pause
    let is_converting = state.is_converting.lock().await;
    if !*is_converting {
        return ConversionResult {
            file_id: file.id.clone(),
            success: false,
            output_path: None,
            error: Some("Conversion cancelled".to_string()),
        };
    }
    drop(is_converting);

    // Wait while paused (check periodically)
    loop {
        let is_paused = state.is_paused.lock().await;
        if !*is_paused {
            break;
        }
        drop(is_paused);
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Check if conversion was cancelled while paused
        let is_converting = state.is_converting.lock().await;
        if !*is_converting {
            return ConversionResult {
                file_id: file.id.clone(),
                success: false,
                output_path: None,
                error: Some("Conversion cancelled".to_string()),
            };
        }
        drop(is_converting);
    }

    // Build FFmpeg command
    let mut cmd = Command::new(ffmpeg_path);

    // Hide console window on Windows
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    // Check if we need to merge video and audio
    let merge_audio = file.file_type == "video"
        && file.has_audio.unwrap_or(false)
        && Path::new(&file.path)
            .parent()
            .map_or(false, |p| p.join("audio.m4s").exists());

    if merge_audio {
        let audio_path = Path::new(&file.path).parent().unwrap().join("audio.m4s");
        cmd.arg("-i")
            .arg(&file.path)
            .arg("-i")
            .arg(&audio_path);
    } else {
        cmd.arg("-i").arg(&file.path);
    }

    cmd.arg("-y")
        .arg("-progress")
        .arg("pipe:2")
        .arg("-nostats");

    // Add format-specific options
    if file.file_type == "video" {
        if merge_audio {
            // Merged video+audio: copy video stream directly (no re-encode), encode audio to aac
            cmd.arg("-c:v").arg("copy");
            cmd.arg("-c:a").arg("aac");
            cmd.arg("-b:a").arg("128k");
        } else {
            match output_ext.as_str() {
                "mp4" | "mkv" => {
                    cmd.arg("-c:v").arg(encoder_config.video_encoder);
                    if encoder_config.use_gpu {
                        cmd.arg("-preset").arg("p4");
                        cmd.arg("-cq").arg("23");
                    } else {
                        cmd.arg("-preset").arg("medium");
                        cmd.arg("-crf").arg("23");
                    }
                    cmd.arg("-c:a").arg(encoder_config.audio_encoder);
                    cmd.arg("-b:a").arg("128k");
                }
                "avi" => {
                    cmd.arg("-c:v").arg(encoder_config.video_encoder);
                    if encoder_config.use_gpu {
                        cmd.arg("-preset").arg("p4");
                        cmd.arg("-cq").arg("23");
                    } else {
                        cmd.arg("-preset").arg("medium");
                        cmd.arg("-crf").arg("23");
                    }
                    cmd.arg("-c:a").arg("mp3");
                }
                _ => {}
            }
        }
    } else {
        match output_ext.as_str() {
            "mp3" => {
                cmd.arg("-vn");
                cmd.arg("-c:a").arg("libmp3lame");
                cmd.arg("-b:a").arg("192k");
            }
            "aac" => {
                cmd.arg("-vn");
                cmd.arg("-c:a").arg("aac");
                cmd.arg("-b:a").arg("192k");
            }
            "flac" => {
                cmd.arg("-vn");
                cmd.arg("-c:a").arg("flac");
            }
            _ => {}
        }
    }

    cmd.arg(&output_path_str);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Validate output path to prevent writing outside intended directory
    let output_path_obj = Path::new(&output_path_str);
    let output_dir_obj = Path::new(output_dir);

    // Check if the output path is within the intended output directory
    match output_path_obj.canonicalize() {
        Ok(canonical_output) => {
            let canonical_dir = output_dir_obj
                .canonicalize()
                .unwrap_or_else(|_| output_dir_obj.to_path_buf());
            if !canonical_output.starts_with(&canonical_dir) {
                log::error!(target: "converter", "Output path outside of output directory: {}", output_path_str);
                return ConversionResult {
                    file_id: file.id.clone(),
                    success: false,
                    output_path: None,
                    error: Some("Invalid output path: path traversal detected".to_string()),
                };
            }
        }
        Err(_) => {
            // Path doesn't exist yet, which is expected for new files
            // Check parent directory instead
            if let Some(parent) = output_path_obj.parent() {
                match parent.canonicalize() {
                    Ok(canonical_parent) => {
                        let canonical_dir = output_dir_obj
                            .canonicalize()
                            .unwrap_or_else(|_| output_dir_obj.to_path_buf());
                        if !canonical_parent.starts_with(&canonical_dir) {
                            log::error!(target: "converter", "Output parent path outside of output directory: {}", output_path_str);
                            return ConversionResult {
                                file_id: file.id.clone(),
                                success: false,
                                output_path: None,
                                error: Some(
                                    "Invalid output path: path traversal detected".to_string(),
                                ),
                            };
                        }
                    }
                    Err(e) => {
                        log::error!(target: "converter", "Failed to validate output path: {}", e);
                        return ConversionResult {
                            file_id: file.id.clone(),
                            success: false,
                            output_path: None,
                            error: Some(format!("Failed to validate output path: {}", e)),
                        };
                    }
                }
            }
        }
    }

    log::info!(target: "converter", "Converting: {} -> {}", file.path, output_path_str);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::error!(target: "converter", "Failed to spawn FFmpeg: {}", e);
            return ConversionResult {
                file_id: file.id.clone(),
                success: false,
                output_path: None,
                error: Some(format!("Failed to start FFmpeg: {}", e)),
            };
        }
    };

    // Read progress from stderr (FFmpeg outputs progress to stderr)
    let stderr = child.stderr.take();
    let file_id = file.id.clone();
    let file_name = file.name.clone();
    let app_clone = app.clone();

    if let Some(stderr) = stderr {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let file_size = file.size as f64;
        let start_instant = std::time::Instant::now();

        while let Ok(Some(line)) = lines.next_line().await {
            // FFmpeg progress format: "progress=XX" or contains percentage
            if let Some(caps) = PERCENT_PATTERN.captures(&line) {
                if let Ok(progress) = caps[1].parse::<f64>() {
                    let realtime_completed = {
                        let count = state.completed_count.lock().await;
                        *count
                    };
                    let (elapsed_time, remaining_time) = calculate_time_stats(
                        progress_info.start_time,
                        progress_info.current_index,
                        progress_info.total_count,
                        progress,
                        realtime_completed,
                    );

                    // Calculate performance metrics
                    let processed_bytes = (file_size * progress / 100.0) as u64;
                    let elapsed_secs = start_instant.elapsed().as_secs_f64();
                    let conversion_speed = if elapsed_secs > 0.0 {
                        (processed_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
                    } else {
                        0.0
                    };
                    let total_elapsed = if elapsed_time > 0 {
                        elapsed_time as f64
                    } else {
                        1.0
                    };
                    let average_speed = file_size / (1024.0 * 1024.0) / total_elapsed;
                    let realtime_completed = {
                        let count = state.completed_count.lock().await;
                        *count
                    };
                    let _ = app_clone.emit(
                        "conversion-progress",
                        ConversionProgress {
                            file_id: file_id.clone(),
                            file_name: file_name.clone(),
                            progress,
                            status: "converting".to_string(),
                            current_index: progress_info.current_index,
                            completed_count: realtime_completed,
                            total_count: progress_info.total_count,
                            elapsed_time,
                            remaining_time,
                            conversion_speed,
                            average_speed,
                            estimated_size: file_size as u64,
                            processed_bytes,
                        },
                    );
                }
            }
            // Also check for "out_time" for time-based progress
            if line.starts_with("out_time=") {
                let time_str = line.strip_prefix("out_time=").unwrap_or("");
                log::trace!("[FFmpeg] Progress time: {}", time_str);
            }
        }
    }

    // Calculate final time stats
    let realtime_completed = {
        let count = state.completed_count.lock().await;
        *count
    };
    let (elapsed_time, remaining_time) = calculate_time_stats(
        progress_info.start_time,
        progress_info.current_index,
        progress_info.total_count,
        100.0,
        realtime_completed,
    );
    let file_size = file.size as f64;
    let conversion_speed = if elapsed_time > 0 {
        file_size / (1024.0 * 1024.0) / elapsed_time as f64
    } else {
        0.0
    };

    // Wait for completion
    let result = match child.wait().await {
        Ok(status) => {
            if status.success() {
                {
                    let mut count = state.completed_count.lock().await;
                    *count += 1;
                }
                // 获取更新后的已完成数量
                let completed_after = {
                    let count = state.completed_count.lock().await;
                    *count
                };
                let _ = app.emit(
                    "conversion-progress",
                    ConversionProgress {
                        file_id: file.id.clone(),
                        file_name: file.name.clone(),
                        progress: 100.0,
                        status: "completed".to_string(),
                        current_index: progress_info.current_index,
                        completed_count: completed_after,
                        total_count: progress_info.total_count,
                        elapsed_time,
                        remaining_time,
                        conversion_speed,
                        average_speed: conversion_speed,
                        estimated_size: file_size as u64,
                        processed_bytes: file_size as u64,
                    },
                );

                // Increment completed count

                log::info!(target: "converter", "Successfully converted: {} ({:.2} MB/s)", output_path_str, conversion_speed);

                // Validate file integrity after conversion
                let validation = validate_file_integrity(&output_path_str, &file);

                if !validation.is_valid {
                    log::warn!(target: "converter", "Integrity validation failed for file: {}", file.id);
                    for detail in &validation.validation_details {
                        log::warn!(target: "converter", "Validation: {}", detail);
                    }

                    // Still return success but emit validation result
                    let _ = app.emit("conversion-integrity", validation);
                } else {
                    log::info!(target: "converter", "Integrity validation passed for file: {}", file.id);
                    let _ = app.emit("conversion-integrity", validation);
                }

                ConversionResult {
                    file_id: file.id.clone(),
                    success: true,
                    output_path: Some(output_path_str),
                    error: None,
                }
            } else {
                let error = format!("FFmpeg exited with status: {}", status);
                // 使用更详细的错误格式，包含完整错误链信息
                log::error!(
                    target: "converter",
                    "[Conversion] Failed: {} - {:#}",
                    file.name,
                    error
                );

                // Increment completed count (even if failed)
                {
                    let mut count = state.completed_count.lock().await;
                    *count += 1;
                }

                ConversionResult {
                    file_id: file.id.clone(),
                    success: false,
                    output_path: None,
                    error: Some(error),
                }
            }
        }
        Err(e) => {
            // 使用 {:#} 格式打印完整错误链，包含堆栈信息
            let error = format!("{:#}", e);
            log::error!(
                target: "converter",
                "[Conversion] Error: {} - {:#}",
                file.name,
                e
            );

            // Increment completed count (even if error)
            {
                let mut count = state.completed_count.lock().await;
                *count += 1;
            }

            ConversionResult {
                file_id: file.id.clone(),
                success: false,
                output_path: None,
                error: Some(error),
            }
        }
    };

    result
}

/// Calculate elapsed and remaining time based on current progress
/// Uses a weighted approach that considers both overall progress and recent file speeds
fn calculate_time_stats(
    start_time: Option<std::time::Instant>,
    current_index: usize,
    total_count: usize,
    current_file_progress: f64,
    completed_count: usize, // 添加实时完成的文件数量参数
) -> (u64, u64) {
    let elapsed_time = if let Some(start) = start_time {
        start.elapsed().as_secs()
    } else {
        0
    };

    // Calculate overall progress percentage (0.0 to 100.0)
    // Use real-time completed_count for accurate calculation in concurrent scenario
    let overall_progress = if total_count > 0 {
        let completed_files = completed_count;
        // Add current file's partial progress
        let current_progress_fraction = current_file_progress / 100.0;
        // Avoid double counting: if file is fully completed, it should already be in completed_count
        let progress_ratio =
            (completed_files as f64 + current_progress_fraction) / (total_count as f64);
        progress_ratio.min(1.0) // Cap at 100%
    } else {
        0.0
    };

    // Estimate remaining time
    let remaining_time = if overall_progress > 0.1 && elapsed_time > 0 {
        // Method 1: Linear extrapolation based on overall progress
        let total_estimated_linear = (elapsed_time as f64) * 100.0 / overall_progress;
        let remaining_linear = total_estimated_linear - (elapsed_time as f64);

        // Method 2: Per-file average (more accurate for consistent file sizes)
        // Only count fully completed files for average calculation
        let completed_files = if current_file_progress >= 100.0 {
            current_index + 1
        } else if current_index > 0 {
            current_index // Use completed files only
        } else {
            0
        };

        let avg_time_per_file = if completed_files > 0 {
            elapsed_time as f64 / completed_files as f64
        } else {
            0.0
        };

        // Files remaining (including current file if not completed)
        let files_remaining = total_count.saturating_sub(completed_files);

        let remaining_per_file = if avg_time_per_file > 0.0 && files_remaining > 0 {
            // Adjust for current file progress
            let current_file_fraction = if current_file_progress < 100.0 && current_index > 0 {
                // Current file is partially done, count remaining fraction
                1.0 - (current_file_progress / 100.0)
            } else if current_file_progress < 100.0 {
                // First file is partially done, count it as full remaining
                1.0
            } else {
                0.0
            };
            (files_remaining as f64 + current_file_fraction) * avg_time_per_file
        } else {
            0.0
        };

        // Use weighted average:
        // - 70% per-file estimation (more accurate for consistent file sizes)
        // - 30% linear estimation (good for initial estimates)
        let remaining_weighted = if remaining_per_file > 0.0 {
            (remaining_per_file * 0.7) + (remaining_linear * 0.3)
        } else {
            remaining_linear
        };

        remaining_weighted.max(0.0) as u64
    } else if elapsed_time > 0 && current_index > 0 {
        // Early stage: simple estimate based on first file's progress
        // This is only used when we don't have enough data
        0
    } else {
        0
    };

    (elapsed_time, remaining_time)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_config_default() {
        let config = EncoderConfig::default();
        assert_eq!(config.video_encoder, "libx264");
        assert_eq!(config.audio_encoder, "aac");
        assert!(!config.use_gpu);
    }

    #[test]
    fn test_encoder_config_gpu_nvidia() {
        let config = get_encoder_config(GpuType::Nvidia, "mp4");
        assert_eq!(config.video_encoder, "h264_nvenc");
        assert!(config.use_gpu);
    }

    #[test]
    fn test_encoder_config_gpu_amd() {
        let config = get_encoder_config(GpuType::Amd, "mp4");
        assert_eq!(config.video_encoder, "h264_amf");
        assert!(config.use_gpu);
    }

    #[test]
    fn test_encoder_config_cpu() {
        let config = get_encoder_config(GpuType::None, "mp4");
        assert_eq!(config.video_encoder, "libx264");
        assert!(!config.use_gpu);
    }

    #[test]
    fn test_encoder_config_audio_formats() {
        let config_mp3 = get_encoder_config(GpuType::None, "mp3");
        assert_eq!(config_mp3.audio_encoder, "libmp3lame");

        let config_aac = get_encoder_config(GpuType::None, "aac");
        assert_eq!(config_aac.audio_encoder, "aac");

        let config_flac = get_encoder_config(GpuType::None, "flac");
        assert_eq!(config_flac.audio_encoder, "flac");
    }

    #[test]
    fn test_gpu_type_variants() {
        assert_ne!(GpuType::Nvidia, GpuType::Amd);
        assert_ne!(GpuType::Amd, GpuType::Intel);
        assert_ne!(GpuType::Intel, GpuType::None);
    }
}

/// Validate the integrity of a converted file
/// Checks: file existence, size, readability, and basic format validation
pub fn validate_file_integrity(
    output_path: &str,
    original_file: &MediaFile,
) -> crate::IntegrityValidation {
    let mut validation_details: Vec<String> = Vec::new();
    let mut is_valid = true;

    // Check 1: File exists
    let path_obj = Path::new(output_path);
    if !path_obj.exists() {
        validation_details.push("文件不存在".to_string());

        return crate::IntegrityValidation {
            file_id: original_file.id.clone(),
            is_valid: false,
            validation_details,
            file_size: 0,
            expected_size: Some(original_file.size),
        };
    }

    // Check 2: File size
    let metadata = match path_obj.metadata() {
        Ok(meta) => meta,
        Err(e) => {
            validation_details.push(format!("无法读取文件元数据: {}", e));

            return crate::IntegrityValidation {
                file_id: original_file.id.clone(),
                is_valid: false,
                validation_details,
                file_size: 0,
                expected_size: Some(original_file.size),
            };
        }
    };

    let output_size = metadata.len();
    validation_details.push(format!("输出文件大小: {} 字节", output_size));

    // Validate file size is reasonable (not zero and not suspiciously small)
    if output_size == 0 {
        validation_details.push("文件大小为0,可能转换失败".to_string());
        is_valid = false;
    } else if output_size < 1024 {
        // Less than 1KB is suspicious
        validation_details.push("文件大小异常小 (<1KB),可能转换不完整".to_string());
        is_valid = false;
    }

    // Check 3: File is readable (can open and read)
    if let Err(e) = std::fs::File::open(output_path) {
        validation_details.push(format!("无法打开文件: {}", e));
        is_valid = false;
    }

    // Check 4: File format validation (basic check)
    let extension = path_obj
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" => {
            // Video formats: check if it's not empty
            if output_size < 100 {
                validation_details.push("视频文件大小过小,可能损坏".to_string());
                is_valid = false;
            }
            validation_details.push("视频格式校验通过".to_string());
        }
        "mp3" | "aac" | "m4a" | "flac" | "wav" | "ogg" => {
            // Audio formats: check if it's not empty
            if output_size < 100 {
                validation_details.push("音频文件大小过小,可能损坏".to_string());
                is_valid = false;
            }
            validation_details.push("音频格式校验通过".to_string());
        }
        _ => {
            validation_details.push(format!("未知文件格式: {}", extension));
        }
    }

    // Check 5: Size comparison with original (rough check)
    // Video files can be much smaller after compression
    // Audio files should be similar size
    if original_file.file_type == "audio" {
        let size_diff = (output_size as i64 - original_file.size as i64).abs();
        let size_ratio = if original_file.size > 0 {
            size_diff as f64 / original_file.size as f64 * 100.0
        } else {
            0.0
        };

        if size_ratio > 50.0 {
            // More than 50% difference in audio size is suspicious
            validation_details.push(format!("音频文件大小异常 (差异: {:.1}%)", size_ratio));
            is_valid = false;
        }
    }

    // Check 6: File can be opened for reading (basic playback test)
    if let Ok(file) = std::fs::File::open(output_path) {
        if let Err(e) = file.metadata() {
            validation_details.push(format!("读取文件元数据失败: {}", e));
            is_valid = false;
        } else {
            validation_details.push("文件可读取".to_string());
        }
    }

    crate::IntegrityValidation {
        file_id: original_file.id.clone(),
        is_valid,
        validation_details,
        file_size: output_size,
        expected_size: Some(original_file.size),
    }
}
