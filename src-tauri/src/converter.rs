use crate::{AppSettings, AppState, ConversionProgress, ConversionResult, MediaFile};
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
}

#[derive(Clone)]
struct ProgressInfo {
    current_index: usize,
    total_count: usize,
    start_time: Option<std::time::Instant>,
}

static PERCENT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d+\.?\d*)%").unwrap()
});

pub async fn get_ffmpeg_path(app: Option<&AppHandle>) -> Result<String, String> {
    // First, check bundled resources
    if let Some(app_handle) = app {
        if let Ok(resource_dir) = app_handle.path().resource_dir() {
            let bundled_ffmpeg = resource_dir.join("ffmpeg.exe");
            if bundled_ffmpeg.exists() {
                let path_str = bundled_ffmpeg.to_string_lossy().to_string();
                if test_ffmpeg_path(&path_str).await {
                    log::info!("Using bundled FFmpeg: {}", path_str);
                    return Ok(path_str);
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
                log::info!("[GPU] Detected NVIDIA GPU - hardware acceleration enabled");
                return GpuType::Nvidia;
            }

            // Check for AMD GPU (amf encoders)
            if combined.contains("h264_amf") || combined.contains("hevc_amf") {
                log::info!("[GPU] Detected AMD GPU - hardware acceleration enabled");
                return GpuType::Amd;
            }

            // Check for Intel GPU (qsv encoders)
            if combined.contains("h264_qsv") || combined.contains("hevc_qsv") {
                log::info!("[GPU] Detected Intel GPU - hardware acceleration enabled");
                return GpuType::Intel;
            }

            log::debug!("[GPU] No hardware acceleration detected - using CPU encoding");
            GpuType::None
        }
        Err(e) => {
            log::warn!("[GPU] Failed to detect GPU: {}", e);
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
        log::error!("Invalid output path: must be absolute path");
        return files
            .iter()
            .map(|f| ConversionResult {
                file_id: f.id.clone(),
                success: false,
                output_path: None,
                error: Some("Invalid output path: must be absolute".to_string()),
            })
            .collect();
    }

    // Ensure output directory exists
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        log::error!("Failed to create output directory: {}", e);
        return files
            .iter()
            .map(|f| ConversionResult {
                file_id: f.id.clone(),
                success: false,
                output_path: None,
                error: Some(format!("Failed to create output directory: {}", e)),
            })
            .collect();
    }

    // Get FFmpeg path
    let ffmpeg_path = match get_ffmpeg_path(Some(&app)).await {
        Ok(path) => path,
        Err(e) => {
            log::error!("FFmpeg not found: {}", e);
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

    log::info!("Using FFmpeg at: {}", ffmpeg_path);

    // Detect GPU type for hardware acceleration
    let gpu_type = detect_gpu_type(&ffmpeg_path).await;
    let encoder_config = get_encoder_config(gpu_type, &settings.output_format_video);

    if encoder_config.use_gpu {
        log::info!("Using GPU acceleration with encoder: {}", encoder_config.video_encoder);
    } else {
        log::info!("Using CPU encoding with encoder: {}", encoder_config.video_encoder);
    }

    // Convert files with concurrency
    let concurrency = settings.concurrency.max(1);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let total_count = files.len();

    let mut handles = Vec::new();

    for (index, file) in files.into_iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
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
        if let Ok(result) = handle.await {
            results.push(result);
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
        let optimized_dir = optimize_directory_structure(rel_dir, base_dir);
        let sub_dir = Path::new(output_dir).join(&optimized_dir);
        if let Err(e) = std::fs::create_dir_all(&sub_dir) {
            log::warn!("Failed to create subdirectory: {}", e);
        }
        sub_dir.join(format!("{}.{}", output_name, output_ext))
    } else {
        Path::new(output_dir).join(format!("{}.{}", output_name, output_ext))
    };

    let output_path_str = output_path.to_string_lossy().to_string();

    // Calculate elapsed and remaining time
    let (elapsed_time, remaining_time) = calculate_time_stats(
        progress_info.start_time,
        progress_info.current_index,
        progress_info.total_count,
        0.0,
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
    if file.file_type == "video" && file.has_audio.unwrap_or(false) {
        // Get parent directory and check for audio.m4s
        if let Some(parent) = Path::new(&file.path).parent() {
            let audio_path = parent.join("audio.m4s");
            if audio_path.exists() {
                // Merge video and audio
                cmd.arg("-i")
                    .arg(&file.path)
                    .arg("-i")
                    .arg(&audio_path)
                    .arg("-c:v").arg("copy")
                    .arg("-c:a").arg("aac")
                    .arg("-y")
                    .arg("-progress")
                    .arg("pipe:2")
                    .arg("-nostats")
                    .arg(&output_path_str);
            } else {
                cmd.arg("-i")
                    .arg(&file.path)
                    .arg("-y")
                    .arg("-progress")
                    .arg("pipe:2")
                    .arg("-nostats");
            }
        } else {
            cmd.arg("-i")
                .arg(&file.path)
                .arg("-y")
                .arg("-progress")
                .arg("pipe:2")
                .arg("-nostats");
        }
    } else {
        cmd.arg("-i")
            .arg(&file.path)
            .arg("-y")
            .arg("-progress")
            .arg("pipe:2")
            .arg("-nostats");
    }

    // Add format-specific options - use GPU encoder if available
    if file.file_type == "video" {
        match output_ext.as_str() {
            "mp4" | "mkv" => {
                // Use GPU encoder if available, otherwise use CPU
                cmd.arg("-c:v").arg(encoder_config.video_encoder);
                if encoder_config.use_gpu {
                    // GPU encoding: use quality preset
                    cmd.arg("-preset").arg("p4");
                    cmd.arg("-cq").arg("23");
                } else {
                    // CPU encoding fallback
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
    } else {
        // Audio
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
            let canonical_dir = output_dir_obj.canonicalize().unwrap_or_else(|_| output_dir_obj.to_path_buf());
            if !canonical_output.starts_with(&canonical_dir) {
                log::error!("Output path outside of output directory: {}", output_path_str);
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
                        let canonical_dir = output_dir_obj.canonicalize().unwrap_or_else(|_| output_dir_obj.to_path_buf());
                        if !canonical_parent.starts_with(&canonical_dir) {
                            log::error!("Output parent path outside of output directory: {}", output_path_str);
                            return ConversionResult {
                                file_id: file.id.clone(),
                                success: false,
                                output_path: None,
                                error: Some("Invalid output path: path traversal detected".to_string()),
                            };
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to validate output path: {}", e);
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

    log::info!("Converting: {} -> {}", file.path, output_path_str);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to spawn FFmpeg: {}", e);
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
                    let (elapsed_time, remaining_time) = calculate_time_stats(
                        progress_info.start_time,
                        progress_info.current_index,
                        progress_info.total_count,
                        progress,
                    );

                    // Calculate performance metrics
                    let processed_bytes = (file_size * progress / 100.0) as u64;
                    let elapsed_secs = start_instant.elapsed().as_secs_f64();
                    let conversion_speed = if elapsed_secs > 0.0 {
                        (processed_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs
                    } else {
                        0.0
                    };
                    let total_elapsed = if elapsed_time > 0 { elapsed_time as f64 } else { 1.0 };
                    let average_speed = file_size / (1024.0 * 1024.0) / total_elapsed;

                    let _ = app_clone.emit(
                        "conversion-progress",
                        ConversionProgress {
                            file_id: file_id.clone(),
                            file_name: file_name.clone(),
                            progress,
                            status: "converting".to_string(),
                            current_index: progress_info.current_index,
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
    let (elapsed_time, remaining_time) = calculate_time_stats(
        progress_info.start_time,
        progress_info.current_index,
        progress_info.total_count,
        100.0,
    );
    let file_size = file.size as f64;
    let conversion_speed = if elapsed_time > 0 { file_size / (1024.0 * 1024.0) / elapsed_time as f64 } else { 0.0 };

    // Wait for completion
    let result = match child.wait().await {
        Ok(status) => {
            if status.success() {
                let _ = app.emit(
                    "conversion-progress",
                    ConversionProgress {
                        file_id: file.id.clone(),
                        file_name: file.name.clone(),
                        progress: 100.0,
                        status: "completed".to_string(),
                        current_index: progress_info.current_index,
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
                {
                    let mut count = state.completed_count.lock().await;
                    *count += 1;
                }

                log::info!("[Conversion] Successfully converted: {} ({:.2} MB/s)", output_path_str, conversion_speed);

                ConversionResult {
                    file_id: file.id.clone(),
                    success: true,
                    output_path: Some(output_path_str),
                    error: None,
                }
            } else {
                let error = format!("FFmpeg exited with status: {}", status);
                log::error!("[Conversion] Failed: {} - {}", file.name, error);

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
            let error = format!("Process error: {}", e);
            log::error!("[Conversion] Error: {} - {}", file.name, error);

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
) -> (u64, u64) {
    let elapsed_time = if let Some(start) = start_time {
        start.elapsed().as_secs()
    } else {
        0
    };

    // Calculate overall progress percentage (0.0 to 100.0)
    let overall_progress = if total_count > 0 {
        ((current_index as f64) * 100.0 + current_file_progress) / (total_count as f64)
    } else {
        0.0
    };

    // Estimate remaining time with improved accuracy
    let remaining_time = if overall_progress > 0.5 && elapsed_time > 0 {
        // Method 1: Linear extrapolation based on progress
        let total_estimated_linear = (elapsed_time as f64) * 100.0 / overall_progress;
        let remaining_linear = total_estimated_linear - (elapsed_time as f64);

        // Method 2: Consider current file's progress for finer estimation
        let files_remaining = total_count.saturating_sub(current_index + 1);
        let current_file_remaining = if current_file_progress < 100.0 {
            1.0 - (current_file_progress / 100.0)
        } else {
            0.0
        };

        // Estimate average time per file
        let avg_time_per_file = if current_index > 0 {
            (elapsed_time as f64) / (current_index as f64 + current_file_progress / 100.0)
        } else {
            0.0
        };

        let remaining_per_file = if avg_time_per_file > 0.0 {
            (files_remaining as f64 + current_file_remaining) * avg_time_per_file
        } else {
            remaining_linear
        };

        // Use weighted average: 60% linear, 40% per-file estimation
        // This gives more weight to overall progress while still considering recent speeds
        let remaining_weighted = (remaining_linear * 0.6) + (remaining_per_file * 0.4);

        remaining_weighted.max(0.0) as u64
    } else {
        // Not enough data to estimate, return 0 or a rough estimate if we have some data
        if elapsed_time > 0 && current_index > 0 {
            // Rough estimate based on completed files only
            let avg_time_per_file = elapsed_time as f64 / current_index as f64;
            let files_remaining = total_count.saturating_sub(current_index);
            (avg_time_per_file * files_remaining as f64).max(0.0) as u64
        } else {
            0
        }
    };

    (elapsed_time, remaining_time)
}

/// Optimize directory structure by skipping single subfolder levels
/// Example: download/v/video.blv -> output to download/result/video.mp4 (skip "v")
/// But: download/v1/video1/blv -> output to download/result/v1/video1.mp4 (keep structure)
/// Extended: download/a/b/video.blv -> output to download/result/video.mp4 (skip both "a" and "b" if each has only one subfolder)
/// IMPORTANT: Always preserve at least the first level (sub-top level directory)
fn optimize_directory_structure(relative_dir: &Path, base_dir: &str) -> PathBuf {
    if relative_dir.components().count() <= 1 {
        // No subdirectory or only one level, keep as is (this IS the sub-top level)
        return relative_dir.to_path_buf();
    }

    let base_path = Path::new(base_dir);
    let mut result_path = relative_dir.to_path_buf();
    let mut current_base = base_path.to_path_buf();

    // Count the original levels to ensure we preserve at least the first one
    let original_components: Vec<_> = relative_dir.components().collect();
    let original_level_count = original_components.len();

    // Only try to skip if we have more than 1 level (need to preserve at least level 0)
    // We can skip levels 1, 2, etc., but never level 0
    let max_levels_to_skip = original_level_count.saturating_sub(1).min(3);

    for skip_level in 0..max_levels_to_skip {
        // Ensure we never skip level 0 (the first directory after base_dir)
        if skip_level >= original_level_count {
            break;
        }

        // Get the first component of the current result path
        let components: Vec<_> = result_path.components().collect();
        if components.is_empty() {
            break;
        }

        let first_component = &components[0];
        let check_path = current_base.join(first_component);
        let display_name = format!("{}", first_component.as_os_str().to_string_lossy());

        // Check if this directory contains only one subdirectory
        if let Ok(entries) = std::fs::read_dir(&check_path) {
            let subdirs: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect();

            if subdirs.len() == 1 && components.len() > 1 {
                // Skip this level and continue checking the next
                result_path = components[1..].iter().collect();
                current_base = check_path;
                log::info!("Optimizing path: skipping single subfolder '{}'", display_name);
            } else {
                // No more single subfolders to skip, or this is the last level
                break;
            }
        } else {
            // Can't read directory, stop optimizing
            break;
        }

        if result_path.as_os_str().is_empty() {
            break;
        }
    }

    result_path
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

