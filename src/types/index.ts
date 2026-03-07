export interface MediaFile {
  id: string;
  path: string;
  name: string;
  size: number;
  file_type: "video" | "audio";
  title: string;
  output_name: string;
  has_audio?: boolean;
}

export interface ConversionProgress {
  file_id: string;
  file_name: string;
  progress: number;
  status: string;
  current_index: number;
  total_count: number;
  elapsed_time: number;
  remaining_time: number;
  // Performance metrics
  conversion_speed: number;    // MB/s
  average_speed: number;       // Average MB/s
  estimated_size: number;      // Estimated output size in bytes
  processed_bytes: number;    // Bytes processed so far
}

export interface ConversionResult {
  file_id: string;
  success: boolean;
  output_path: string | null;
  error: string | null;
}

export interface ScanResult {
  files: MediaFile[];
  total_size: number;
}

export interface ScanProgress {
  found_files: number;
  current_path: string;
}

export interface AppSettings {
  sound_enabled: boolean;
  output_format_video: string;
  output_format_audio: string;
  output_path: string;
  concurrency: number;
}

export interface ConversionCompleteEvent {
  success_count: number;
  total_count: number;
  results: ConversionResult[];
}

export interface ConversionCancelledEvent {
  completed_count: number;
  total_count: number;
}
