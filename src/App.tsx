import React, { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import * as dialog from '@tauri-apps/plugin-dialog';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Progress } from '@/components/ui/progress';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  FolderOpen,
  Video,
  Music,
  Play,
  Square,
  Pause,
  Settings,
  CheckCircle,
  XCircle,
  ExternalLink,
  Folder,
  Volume2,
  VolumeX,
  AlertTriangle,
} from 'lucide-react';
import type {
  MediaFile,
  ConversionProgress,
  AppSettings,
  ScanResult,
  ScanProgress,
  ConversionCompleteEvent,
  ConversionCancelledEvent,
  IntegrityValidation,
  FileStatus,
} from '@/types';
import { formatFileSize, formatTime } from '@/lib/utils';
import { createThrottledState } from '@/hooks/useThrottle';
import { useVirtualList } from '@/hooks/useVirtualList';
import { toast } from 'sonner';
import { isTauri } from '@tauri-apps/api/core';

// ========== 常量定义 ==========
const THROTTLE_INTERVAL = 150; // 节流间隔 (ms)
const FLUSH_INTERVAL = 200; // 刷新间隔 (ms)
const VIRTUAL_LIST_THRESHOLD = 100; // 虚拟列表阈值
// const PROGRESS_THROTTLE = 150; // 进度更新节流

// ========== 类型定义 ==========

// interface DialogState {
//   showSettings: boolean;
//   showComplete: boolean;
//   showCancel: boolean;
//   showOpenFolder: boolean;
//   showIntegrity: boolean;
// }

// ========== 辅助函数 ==========

/**
 * 解析对话框返回值
 */
function parseDialogPath(selected: string | string[] | unknown | null): string | null {
  if (!selected) return null;
  if (typeof selected === 'string') return selected.trim();
  if (Array.isArray(selected)) return selected[0]?.trim() || null;
  if (selected && typeof selected === 'object' && 'path' in selected) {
    return (selected as { path?: string }).path?.trim() || null;
  }
  return null;
}

/**
 * 验证路径格式
 */
function validatePath(path: string): { valid: boolean; error?: string } {
  const trimmed = path.trim();
  if (!trimmed) return { valid: false, error: '路径无效' };

  // 检查路径格式
  if (!/^([a-zA-Z]:\\|\\\\|\/)/.test(trimmed)) {
    return { valid: false, error: '路径格式无效' };
  }

  // 检查路径是否包含非法字符
  const invalidChars = /[<>"|?*]/;
  if (invalidChars.test(trimmed)) {
    return { valid: false, error: '路径包含非法字符' };
  }

  // 检查路径长度限制 (Windows MAX_PATH = 260)
  if (trimmed.length > 260) {
    return { valid: false, error: '路径过长' };
  }

  return { valid: true };
}

/**
 * 验证并发数
 */
function validateConcurrency(value: number): boolean {
  return [1, 2, 4, 6, 8].includes(value);
}

// Status display helper
const getStatusDisplay = (status: FileStatus | undefined) => {
  switch (status) {
    case 'pending':
      return { label: '待转换', color: 'text-[#8B949E]' };
    case 'converting':
      return { label: '转换中', color: 'text-[#00D9FF]' };
    case 'validating':
      return { label: '校验中', color: 'text-[#F59E0B]' };
    case 'completed':
      return { label: '已完成', color: 'text-[#10B981]' };
    case 'failed':
      return { label: '转换失败', color: 'text-[#EF4444]' };
    default:
      return { label: '待转换', color: 'text-[#8B949E]' };
  }
};

// Memoized file item component for performance
const FileItem = React.memo(
  ({ file, status }: { file: MediaFile; status?: FileStatus }) => {
    const statusDisplay = getStatusDisplay(status);
    return (
      <div
        className={`flex items-center gap-2 p-1.5 rounded bg-[#161B22]/50 hover:bg-[#161B22] transition-colors ${
          status === 'converting' ? 'ring-1 ring-[#00D9FF]/50 bg-[#00D9FF]/5' : ''
        }`}
      >
        {file.file_type === 'video' ? (
          <Video className="w-3 h-3 text-[#00D9FF] flex-shrink-0" />
        ) : (
          <Music className="w-3 h-3 text-[#8B5CF6] flex-shrink-0" />
        )}
        <div className="flex-1 min-w-0">
          <p className="text-xs truncate" title={file.title}>
            {file.title || file.name}
          </p>
          <p className="text-[10px] text-[#8B949E] truncate" title={file.path}>
            {file.path}
          </p>
        </div>
        <span className={`text-[10px] ${statusDisplay.color} flex-shrink-0 ml-1`}>
          {statusDisplay.label}
        </span>
        <span className="text-[10px] text-[#8B949E] flex-shrink-0">
          {formatFileSize(file.size)}
        </span>
      </div>
    );
  },
  (prevProps, nextProps) => {
    // Custom comparison function for React.memo
    // Only re-render if file or status actually changed
    return (
      prevProps.file.id === nextProps.file.id &&
      prevProps.file.title === nextProps.file.title &&
      prevProps.file.name === nextProps.file.name &&
      prevProps.file.path === nextProps.file.path &&
      prevProps.file.size === nextProps.file.size &&
      prevProps.file.file_type === nextProps.file.file_type &&
      prevProps.status === nextProps.status
    );
  }
);

FileItem.displayName = 'FileItem';

function App() {
  const [folderPath, setFolderPath] = useState<string>('');
  const [outputPath, setOutputPath] = useState<string>('');
  const [files, setFiles] = useState<MediaFile[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [isConverting, setIsConverting] = useState(false);
  const [progress, setProgress] = useState<ConversionProgress | null>(null);
  const [settings, setSettings] = useState<AppSettings>({
    sound_enabled: true,
    output_format_video: 'mp4',
    output_format_audio: 'mp3',
    output_path: '',
    concurrency: 4,
  });
  const [totalSize, setTotalSize] = useState(0);
  const [showSettingsDialog, setShowSettingsDialog] = useState(false);
  const [showCompleteDialog, setShowCompleteDialog] = useState(false);
  const [completeEvent, setCompleteEvent] = useState<ConversionCompleteEvent | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [defaultOutputPath, setDefaultOutputPath] = useState<string>('');
  const [showCancelDialog, setShowCancelDialog] = useState(false);
  const [cancelledEvent, setCancelledEvent] = useState<ConversionCancelledEvent | null>(null);
  const [showOpenFolderDialog, setShowOpenFolderDialog] = useState(false);
  const [isPaused, setIsPaused] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [integrityValidations, setIntegrityValidations] = useState<IntegrityValidation[]>([]);
  const [fileStatuses, setFileStatuses] = useState<Record<string, FileStatus>>({});
  const [settingsLoaded, setSettingsLoaded] = useState(false);

  // Find the minimum current_index among all converting files (for concurrent conversion)
  const getTopConvertingIndex = useCallback(() => {
    if (!progress) return -1;

    // Find all files with 'converting' status and get the minimum index
    let minIndex = -1;
    files.forEach((file, index) => {
      if (fileStatuses[file.id] === 'converting') {
        if (minIndex === -1 || index < minIndex) {
          minIndex = index;
        }
      }
    });

    return minIndex;
  }, [progress, files, fileStatuses]);

  const topConvertingIndex = getTopConvertingIndex();

  // Core functions that need to be defined before their dependencies
  const scanFolder = useCallback(async (path: string) => {
    setIsScanning(true);
    setError(null);
    setFiles([]);
    setTotalSize(0);
    setScanProgress(null);
    setFileStatuses({});

    // 设置扫描超时（60秒，提供用户反馈）
    const timeoutId = setTimeout(() => {
      console.warn('[扫描] 超过 60 秒，扫描可能较慢');
      setError('扫描时间较长，请耐心等待...');
    }, 60000); // 60 秒

    try {
      // 监听扫描进度事件
      const unlisten = await listen<ScanProgress>('scan-progress', (event) => {
        console.log('[扫描进度]', event.payload);
        setScanProgress({
          processed: event.payload.processed,
          total: event.payload.total || 0,
          message: event.payload.message,
        });
      });

      const result = await invoke<ScanResult>('scan_folder', { folderPath: path });
      console.log('[扫描结果] 找到', result.files.length, '个文件，总大小:', result.total_size);

      // 检查扫描结果
      if (result.files.length === 0) {
        setError('该文件夹中没有找到Bilibili缓存文件');
      } else {
        setFiles(result.files);
        setTotalSize(result.total_size);
      }

      // 清理事件监听
      await unlisten();
    } catch (err) {
      console.error('Error scanning folder:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(`扫描失败：${errorMessage}`);
    } finally {
      clearTimeout(timeoutId);
      setIsScanning(false);
      setScanProgress(null);
    }
  }, []);

  const updateSettings = useCallback(
    async (newSettings: Partial<AppSettings>) => {
      try {
        // Validate concurrency value BEFORE updating state
        if (
          newSettings.concurrency !== undefined &&
          !validateConcurrency(newSettings.concurrency)
        ) {
          console.error('Invalid concurrency value:', newSettings.concurrency);
          toast.error(`无效的并发数：${newSettings.concurrency}`);
          return;
        }

        // Use functional update to avoid stale closure
        setSettings((prev) => {
          const updated = { ...prev, ...newSettings };
          // Update backend asynchronously
          invoke('update_settings', { newSettings: updated }).catch((err) => {
            console.error('Failed to update settings:', err);
            toast.error('同步设置失败');
          });
          return updated;
        });
      } catch (err) {
        console.error('Error updating settings:', err);
        toast.error('更新设置失败');
      }
    },
    [] // Removed settings dependency
  );

  // Use virtual list for file rendering when there are many files
  const virtualList = useVirtualList({
    items: files,
    itemHeight: 36,
    overscan: 5,
  });

  // Auto-scroll to the top converting file (useEffect for reliability)
  useEffect(() => {
    if (topConvertingIndex === -1 || !isConverting) return;
    
    // For virtual list (large file count)
    if (files.length > VIRTUAL_LIST_THRESHOLD && virtualList.virtualItems.length > 0) {
      virtualList.scrollToIndex(topConvertingIndex);
    } else {
      // For regular list, use DOM query
      const element = document.querySelector(`[data-file-index="${topConvertingIndex}"]`);
      if (element) {
        element.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [topConvertingIndex, isConverting, files.length, virtualList]);

  // Debug: Log virtual list state
  useEffect(() => {
    if (files.length > 0 && files.length > VIRTUAL_LIST_THRESHOLD) {
      console.log(
        '[虚拟列表] ✅ 文件数:',
        files.length,
        '| virtualItems:',
        virtualList.virtualItems?.length || 0,
        '| totalSize:',
        virtualList.totalSize
      );

      // 检查渲染条件
      const hasVirtualItems = virtualList.virtualItems && virtualList.virtualItems.length > 0;
      console.log('[渲染条件] virtualItems 存在:', !!virtualList.virtualItems);
      console.log('[渲染条件] virtualItems.length:', virtualList.virtualItems?.length);
      console.log('[渲染条件] 条件结果 (&&):', hasVirtualItems);
      console.log('[渲染条件] 应该渲染:', hasVirtualItems ? '✅ 是' : '❌ 否');

      if (!virtualList.virtualItems || virtualList.virtualItems.length === 0) {
        console.warn('[虚拟列表] ⚠️ virtualItems 为空 - 这是正常的初始化延迟');
      }
    }
  }, [
    files.length,
    virtualList.totalSize,
    virtualList.virtualItems?.length,
    virtualList.parentRef,
  ]);

  // Debug: Log Select component state
  useEffect(() => {
    console.log('[Select 调试] settingsLoaded:', settingsLoaded);
    console.log('[Select 调试] settings.concurrency:', settings.concurrency);
    console.log('[Select 调试] String(settings.concurrency):', String(settings.concurrency));
    console.log('[Select 调试] typeof settings.concurrency:', typeof settings.concurrency);
  }, [settingsLoaded, settings.concurrency]);

  // Drag and drop handlers
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    // Only set dragging to false if we're leaving the main container
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX;
    const y = e.clientY;
    if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) {
      setIsDragging(false);
    }
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);

      // Get dropped files/folders
      const items = e.dataTransfer.items;
      if (items && items.length > 0) {
        // Try to get folder path from dropped items
        for (let i = 0; i < items.length; i++) {
          const item = items[i];
          if (item.kind === 'file') {
            const entry = item.webkitGetAsEntry?.();
            if (entry?.isDirectory) {
              // For Tauri, we need to use the file path
              const file = e.dataTransfer.files[i];
              if (file && 'path' in file) {
                const path = (file as File & { path: string }).path;
                if (path) {
                  setFolderPath(path);
                  try {
                    const defaultPath = await invoke<string>('get_default_output_path', {
                      folderPath: path,
                    });
                    setDefaultOutputPath(defaultPath);
                  } catch (err) {
                    console.error('Error getting default output path:', err);
                  }
                  await scanFolder(path);
                  return;
                }
              }
            }
          }
        }
      }
      setError('请拖拽一个文件夹');
    },
    [scanFolder]
  );

  const selectFolder = useCallback(async () => {
    try {
      const selected = await dialog.open({
        directory: true,
        multiple: false,
        title: '选择Bilibili缓存文件夹',
      });

      const path = parseDialogPath(selected);
      if (!path) {
        setError('未选择文件夹');
        return;
      }

      // Validate path format
      const validation = validatePath(path);
      if (!validation.valid) {
        setError(validation.error || '路径无效');
        return;
      }

      setFolderPath(path);
      try {
        const defaultPath = await invoke<string>('get_default_output_path', {
          folderPath: path,
        });
        setDefaultOutputPath(defaultPath);
      } catch (err) {
        console.error('Error getting default output path:', err);
      }
      await scanFolder(path);
    } catch (err) {
      console.error('Error selecting folder:', err);
      setError('选择文件夹失败: ' + String(err));
    }
  }, [scanFolder]);

  const selectOutputFolder = useCallback(async () => {
    try {
      const selected = await dialog.open({
        directory: true,
        multiple: false,
        title: '选择输出文件夹',
      });

      const path = parseDialogPath(selected);
      if (!path) {
        setError('未选择文件夹');
        return;
      }

      // Validate path format
      const validation = validatePath(path);
      if (!validation.valid) {
        setError(validation.error || '路径无效');
        return;
      }

      await invoke('ensure_output_directory', { path });
      setOutputPath(path);
      await updateSettings({ output_path: path });
    } catch (err) {
      console.error('Error selecting output folder:', err);
      setError('选择输出文件夹失败: ' + String(err));
    }
  }, [updateSettings]);

  const startConversion = useCallback(async () => {
    if (files.length === 0) return;
    setIsConverting(true);
    setError(null);
    setCompleteEvent(null);
    setFileStatuses({});
    // Set initial pending status for all files
    const initialStatuses: Record<string, FileStatus> = {};
    files.forEach((file) => {
      initialStatuses[file.id] = 'pending';
    });
    setFileStatuses(initialStatuses);
    try {
      await invoke('start_conversion', { files, folderPath });
    } catch (err) {
      console.error('Error starting conversion:', err);
      setError('开始转换失败: ' + err);
      setIsConverting(false);
    }
  }, [files, folderPath]);

  const cancelConversion = useCallback(async () => {
    try {
      const result = await invoke<ConversionCancelledEvent>('cancel_conversion');
      setCancelledEvent(result);
      setShowCancelDialog(true);
      setIsConverting(false);
      setIsPaused(false);
      setProgress(null);
    } catch (err) {
      console.error('Error cancelling conversion:', err);
    }
  }, []);

  const pauseConversion = useCallback(async () => {
    try {
      await invoke('pause_conversion');
      setIsPaused(true);
    } catch (err) {
      console.error('Error pausing conversion:', err);
      setError('暂停失败: ' + String(err));
    }
  }, []);

  const resumeConversion = useCallback(async () => {
    try {
      await invoke('resume_conversion');
      setIsPaused(false);
    } catch (err) {
      console.error('Error resuming conversion:', err);
      setError('恢复失败: ' + String(err));
    }
  }, []);

  const openOutputFolder = useCallback(async () => {
    const path = outputPath || settings.output_path;
    if (path) {
      // 已设置输出文件夹,直接打开
      try {
        await invoke('open_output_folder', { folderPath: path });
      } catch (err) {
        console.error('Error opening folder:', err);
        setError('打开文件夹失败: ' + err);
      }
    } else if (folderPath) {
      // 已设置输入文件夹,使用默认输出路径
      if (defaultOutputPath) {
        // 有默认路径,弹出确认对话框
        setShowOpenFolderDialog(true);
      } else {
        // 没有默认路径,直接尝试打开输入文件夹的result子目录
        try {
          const resultPath = await invoke<string>('get_default_output_path', {
            folderPath: folderPath,
          });
          setDefaultOutputPath(resultPath);
          await invoke('open_output_folder', { folderPath: resultPath });
        } catch (err) {
          console.error('Error opening folder:', err);
          setError('打开文件夹失败: ' + err);
        }
      }
    } else {
      // 既没有设置输出文件夹,也没有输入文件夹
      setError('请先选择源文件夹');
    }
  }, [outputPath, settings.output_path, folderPath, defaultOutputPath]);

  const openDefaultOutputFolder = useCallback(async () => {
    try {
      // 确保目录存在
      await invoke('ensure_output_directory', { path: defaultOutputPath });
      // 打开文件夹
      await invoke('open_output_folder', { folderPath: defaultOutputPath });
      setShowOpenFolderDialog(false);
    } catch (err) {
      console.error('Error opening default folder:', err);
      setError('打开默认文件夹失败: ' + err);
    }
  }, [defaultOutputPath]);

  // Create throttled state for progress updates (150ms throttle interval)
  // 使用 useRef 保存节流状态实例，确保在整个组件生命周期内保持一致
  const progressThrottleRef = useRef<ReturnType<
    typeof createThrottledState<ConversionProgress | null>
  > | null>(null);

  // 初始化（仅在 ref 为空时创建）
  if (progressThrottleRef.current === null) {
    progressThrottleRef.current = createThrottledState<ConversionProgress | null>(
      null,
      THROTTLE_INTERVAL
    );
  }

  useEffect(() => {
    const initApp = async () => {
      // 检查 Tauri 是否可用
      if (!isTauri()) {
        console.warn('Running outside Tauri environment');
        return;
      }
      // Load settings on mount
      const loadSettings = async () => {
        try {
          const loadedSettings = await invoke<AppSettings>('get_settings');
          setSettings(loadedSettings);
          if (loadedSettings.output_path) {
            setOutputPath(loadedSettings.output_path);
          }
          setSettingsLoaded(true);
          console.log('[初始化] 设置加载完成');
        } catch (err) {
          console.error('Error loading settings:', err);
          // 即使加载失败也设置为 true，使用默认值
          setSettingsLoaded(true);
        }
      };
      loadSettings();

      // Register all event listeners
      const unlisteners: UnlistenFn[] = [];

      // Progress listener with throttling
      listen<ConversionProgress>('conversion-progress', (event) => {
        try {
          const progressData = event.payload;

          // Update file status to 'converting' for the current file
          setFileStatuses((prev) => ({
            ...prev,
            [progressData.file_id]: 'converting',
          }));

          if (!progressThrottleRef.current) return;
          const applied = progressThrottleRef.current.setValue(progressData);
          if (applied !== null) {
            setProgress(applied);
          }
        } catch (err) {
          console.error('Error handling progress:', err);
        }
      }).then((fn) => unlisteners.push(fn));
      // Periodic flush for throttled state
      const flushInterval = setInterval(() => {
        if (!progressThrottleRef.current) return;
        const value = progressThrottleRef.current.forceFlush();
        if (value !== null) {
          setProgress(value);
        }
      }, FLUSH_INTERVAL);

      // Scan progress listener
      listen<ScanProgress>('scan-progress', (event) => {
        setScanProgress(event.payload);
      }).then((fn) => unlisteners.push(fn));

      // Conversion complete listener
      listen<ConversionCompleteEvent>('conversion-complete', (event) => {
        const { success_count, total_count, results } = event.payload;
        toast.success(`转换完成`, {
          description: `成功转换 ${success_count} / ${total_count} 个文件`,
        });
        // Update file statuses based on results
        setFileStatuses((prev) => {
          const newStatuses = { ...prev };
          results.forEach((result) => {
            newStatuses[result.file_id] = result.success ? 'completed' : 'failed';
          });
          return newStatuses;
        });
        setCompleteEvent(event.payload);
        setShowCompleteDialog(true);
        setIsConverting(false);
        setIsPaused(false);
        setProgress(null);
      }).then((fn) => unlisteners.push(fn));

      // Pause/Resume listeners
      listen('conversion-paused', () => setIsPaused(true)).then((fn) => unlisteners.push(fn));
      listen('conversion-resumed', () => setIsPaused(false)).then((fn) => unlisteners.push(fn));

      // Integrity validation listener - silent update, no individual toast
      listen<IntegrityValidation>('conversion-integrity', (event) => {
        const validation = event.payload;
        setIntegrityValidations((prev) => [...prev, validation]);
        // Update file status silently
        setFileStatuses((prev) => ({
          ...prev,
          [validation.file_id]: validation.is_valid ? 'completed' : 'failed',
        }));
        // No individual toast - summary shown in completion dialog
      }).then((fn) => unlisteners.push(fn));

      // Conversion cancelled listener
      listen<ConversionCancelledEvent>('conversion-cancelled', (event) => {
        const { completed_count, total_count } = event.payload;
        toast.info(`转换已取消`, {
          description: `已转换 ${completed_count} / ${total_count} 个文件`,
        });
        setCancelledEvent(event.payload);
        setShowCancelDialog(true);
        setIsConverting(false);
        setIsPaused(false);
        setProgress(null);
      }).then((fn) => unlisteners.push(fn));

      // Cleanup: clear interval and all listeners
      return () => {
        clearInterval(flushInterval);
        unlisteners.forEach((fn) => fn());
      };
    };
    initApp();
  }, []);

  return (
    <div
      className={`h-screen bg-gradient-to-br from-[#0D1117] via-[#161B22] to-[#0D1117] text-white p-4 flex flex-col overflow-hidden transition-all duration-200 ${
        isDragging ? 'ring-2 ring-[#00D9FF] ring-inset' : ''
      }`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {/* Drag overlay */}
      {isDragging && (
        <div className="absolute inset-0 bg-[#0D1117]/90 flex items-center justify-center z-50 pointer-events-none">
          <div className="text-center">
            <FolderOpen className="w-16 h-16 text-[#00D9FF] mx-auto mb-4" />
            <p className="text-xl font-medium text-[#00D9FF]">拖放文件夹到此处</p>
            <p className="text-sm text-[#8B949E] mt-2">释放以选择Bilibili缓存文件夹</p>
          </div>
        </div>
      )}

      {/* Header */}
      <header className="flex items-center justify-between mb-4 flex-shrink-0">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-[#00D9FF] to-[#8B5CF6] flex items-center justify-center">
            <Video className="w-4 h-4 text-white" />
          </div>
          <div>
            <h1 className="text-lg font-bold bg-gradient-to-r from-[#00D9FF] to-[#8B5CF6] bg-clip-text text-transparent">
              Bilibili缓存转换器
            </h1>
            <p className="text-[10px] text-[#8B949E]">高性能音视频格式转换</p>
          </div>
        </div>
        <Button variant="ghost" size="icon" onClick={() => setShowSettingsDialog(true)}>
          <Settings className="w-4 h-4" />
        </Button>
      </header>

      {/* Main Content */}
      <div className="flex-1 overflow-y-auto space-y-3 pr-1">
        {/* Folder Selection */}
        <div className="bg-[#21262D]/80 backdrop-blur-sm rounded-lg p-3 border border-[#30363D]/50 flex-shrink-0">
          <div className="flex flex-wrap gap-2 mb-2">
            <Button
              onClick={selectFolder}
              disabled={isScanning || isConverting}
              className="flex-shrink-0 text-sm h-8"
            >
              <FolderOpen className="w-3 h-3 mr-1.5" />
              选择输入文件夹
            </Button>
            <div className="flex-1 min-w-[150px] flex items-center">
              <p className="text-xs text-[#8B949E] truncate">
                {folderPath || '请选择Bilibili缓存文件夹'}
              </p>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              onClick={selectOutputFolder}
              disabled={isScanning || isConverting}
              variant="outline"
              className="flex-shrink-0 text-sm h-8"
            >
              <FolderOpen className="w-3 h-3 mr-1.5" />
              选择输出文件夹
            </Button>
            <div className="flex-1 min-w-[150px] flex items-center">
              <p className="text-xs text-[#8B949E] truncate">
                {outputPath
                  ? `输出文件夹: ${outputPath}`
                  : folderPath
                    ? `默认输出文件夹: ${defaultOutputPath}`
                    : '请先选择源文件夹'}
              </p>
            </div>
          </div>
        </div>

        {/* File List */}
        <div className="bg-[#21262D]/80 backdrop-blur-sm rounded-lg p-3 border border-[#30363D]/50 flex-1 min-h-[160px] flex flex-col">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-1.5">
              <Video className="w-3 h-3 text-[#00D9FF]" />
              <span className="text-xs font-medium">待转换文件</span>
              <span className="text-[10px] text-[#8B949E]">({files.length}个)</span>
            </div>
            {totalSize > 0 && (
              <span className="text-[10px] text-[#8B949E]">{formatFileSize(totalSize)}</span>
            )}
          </div>

          <div
            ref={virtualList.parentRef}
            className="overflow-y-auto space-y-1 flex-1 min-h-[120px] max-h-[35vh]"
          >
            {isScanning ? (
              <div className="text-center py-6 text-[#8B949E]">
                <div className="animate-spin w-5 h-5 border-2 border-[#00D9FF] border-t-transparent rounded-full mx-auto mb-2" />
                <p className="text-xs">扫描中...</p>
                {scanProgress && (
                  <p className="text-[10px] mt-1">已扫描 {scanProgress.processed} 个文件</p>
                )}
              </div>
            ) : files.length === 0 ? (
              <div className="text-center py-6 text-[#8B949E]">
                <FolderOpen className="w-6 h-6 mx-auto mb-2 opacity-50" />
                <p className="text-xs">
                  {error ? '扫描失败，请重新选择文件夹' : '请选择包含Bilibili缓存的文件夹'}
                </p>
                {error && <p className="text-[10px] mt-1 text-[#EF4444]">{error}</p>}
              </div>
            ) : files.length > VIRTUAL_LIST_THRESHOLD ? (
              // Use virtual scrolling for large file lists (> 100 items)
              <div style={{ height: virtualList.totalSize, position: 'relative' }}>
                {virtualList.virtualItems && virtualList.virtualItems.length > 0 ? (
                  virtualList.virtualItems.map((virtualRow) => {
                    const file = files[virtualRow.index];
                    if (!file) {
                      console.warn(`[虚拟列表] 文件未找到：index=${virtualRow.index}`);
                      return null;
                    }
                    return (
                      <div
                        key={virtualRow.key}
                        data-file-index={virtualRow.index}
                        style={{
                          position: 'absolute',
                          top: 0,
                          left: 0,
                          width: '100%',
                          height: `${virtualRow.size}px`,
                          transform: `translateY(${virtualRow.start}px)`,
                        }}
                      >
                        <FileItem file={file} status={fileStatuses[file.id]} />
                      </div>
                    );
                  })
                ) : (
                  <div className="text-center py-6 text-[#8B949E]">
                    <p className="text-xs">虚拟列表加载中... ({files.length} 个文件)</p>
                  </div>
                )}
              </div>
            ) : (
              // Regular rendering for small file lists
              files.map((file, index) => (
                <div
                  key={file.id}
                  data-file-index={index}
                >
                  <FileItem file={file} status={fileStatuses[file.id]} />
                </div>
              ))
            )}
          </div>
        </div>

        {/* Format Selection */}
        <div className="grid grid-cols-2 gap-2 flex-shrink-0">
          <div className="bg-[#21262D]/80 backdrop-blur-sm rounded-lg p-2 border border-[#30363D]/50">
            <Label className="text-[10px] text-[#8B949E] mb-1 block">视频格式</Label>
            <Select
              value={settings.output_format_video}
              onValueChange={(v) => updateSettings({ output_format_video: v })}
              disabled={isConverting}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="mp4">MP4</SelectItem>
                <SelectItem value="mkv">MKV</SelectItem>
                <SelectItem value="avi">AVI</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="bg-[#21262D]/80 backdrop-blur-sm rounded-lg p-2 border border-[#30363D]/50">
            <Label className="text-[10px] text-[#8B949E] mb-1 block">音频格式</Label>
            <Select
              value={settings.output_format_audio}
              onValueChange={(v) => updateSettings({ output_format_audio: v })}
              disabled={isConverting}
            >
              <SelectTrigger className="h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="mp3">MP3</SelectItem>
                <SelectItem value="aac">AAC</SelectItem>
                <SelectItem value="flac">FLAC</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        {/* Progress */}
        {isConverting && progress && (
          <div className="bg-[#21262D]/80 backdrop-blur-sm rounded-lg p-2 border border-[#30363D]/50 flex-shrink-0">
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-xs text-[#00D9FF] whitespace-nowrap">
                任务进度:{' '}
                {progress.total_count > 0
                  ? Math.round(
                      ((progress.completed_count + progress.progress / 100) /
                        progress.total_count) *
                        100
                    )
                  : 0}
                %
              </span>
              <span className="text-xs text-[#8B949E] whitespace-nowrap">
                {progress.completed_count} / {progress.total_count} 已完成
              </span>
            </div>
            <Progress
              value={
                progress.total_count > 0
                  ? ((progress.completed_count + progress.progress / 100) / progress.total_count) *
                    100
                  : 0
              }
            />

            {/* Detailed progress info */}
            <div className="flex items-center justify-end mt-1.5 text-[10px] text-[#8B949E]">
              <div className="flex items-center gap-3">
                <div className="flex items-center gap-1">
                  <span>已用时:</span>
                  <span>{formatTime(progress.elapsed_time)}</span>
                </div>
                {progress.remaining_time > 0 && (
                  <div className="flex items-center gap-1">
                    <span>预计剩余:</span>
                    <span className="text-[#10B981]">{formatTime(progress.remaining_time)}</span>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Error Display */}
        {error && (
          <div className="bg-[#EF4444]/10 border border-[#EF4444]/50 rounded-lg p-2 flex-shrink-0">
            <p className="text-xs text-[#EF4444]">{error}</p>
          </div>
        )}

        {/* Actions */}
        <div className="flex flex-wrap gap-2 flex-shrink-0 pb-2">
          {!isConverting ? (
            <Button
              onClick={startConversion}
              disabled={files.length === 0}
              className="flex-1 min-w-[140px] text-sm h-9"
            >
              <Play className="w-3.5 h-3.5 mr-1.5" />
              开始转换
            </Button>
          ) : isPaused ? (
            <>
              <Button onClick={resumeConversion} className="flex-1 min-w-[100px] text-sm h-9">
                <Play className="w-3.5 h-3.5 mr-1.5" />
                继续
              </Button>
              <Button
                onClick={cancelConversion}
                variant="destructive"
                className="flex-1 min-w-[100px] text-sm h-9"
              >
                <Square className="w-3.5 h-3.5 mr-1.5" />
                取消
              </Button>
            </>
          ) : (
            <>
              <Button
                onClick={pauseConversion}
                variant="outline"
                className="flex-1 min-w-[100px] text-sm h-9"
              >
                <Pause className="w-3.5 h-3.5 mr-1.5" />
                暂停
              </Button>
              <Button
                onClick={cancelConversion}
                variant="destructive"
                className="flex-1 min-w-[100px] text-sm h-9"
              >
                <Square className="w-3.5 h-3.5 mr-1.5" />
                取消
              </Button>
            </>
          )}
          <Button
            variant="outline"
            onClick={openOutputFolder}
            disabled={!folderPath}
            className="h-9 w-9 p-0"
          >
            <Folder className="w-3.5 h-3.5" />
          </Button>
        </div>
      </div>

      {/* Completion Dialog */}
      <Dialog open={showCompleteDialog} onOpenChange={setShowCompleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>转换完成</DialogTitle>
            <DialogDescription>
              成功转换 {completeEvent?.success_count || 0} / {completeEvent?.total_count || 0}{' '}
              个文件
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2 max-h-60 overflow-y-auto">
            {completeEvent?.results.map((result, idx) => {
              // Find integrity validation for this file
              const validation = integrityValidations.find((v) => v.file_id === result.file_id);
              const hasIssues = validation && !validation.is_valid;

              return (
                <div key={idx} className="flex items-center gap-2 text-sm">
                  {result.success ? (
                    <CheckCircle className="w-4 h-4 text-[#10B981]" />
                  ) : (
                    <XCircle className="w-4 h-4 text-[#EF4444]" />
                  )}
                  <div className="flex-1 min-w-0">
                    <p className="truncate">{result.file_id}</p>
                    {hasIssues && (
                      <p className="text-[10px] text-[#EF4444] truncate flex items-center gap-1">
                        <AlertTriangle className="w-3 h-3 flex-shrink-0" />
                        校验失败
                      </p>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowCompleteDialog(false)}>
              关闭
            </Button>
            <Button onClick={openOutputFolder}>
              <ExternalLink className="w-4 h-4 mr-2" />
              打开文件夹
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Cancel Dialog */}
      <Dialog open={showCancelDialog} onOpenChange={setShowCancelDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>转换已取消</DialogTitle>
            <DialogDescription>
              已完成 {cancelledEvent?.completed_count || 0} / {cancelledEvent?.total_count || 0}{' '}
              个文件
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowCancelDialog(false)}>
              关闭
            </Button>
            <Button onClick={openOutputFolder}>
              <ExternalLink className="w-4 h-4 mr-2" />
              查看
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Open Default Folder Dialog */}
      <Dialog open={showOpenFolderDialog} onOpenChange={setShowOpenFolderDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>打开默认输出文件夹</DialogTitle>
            <DialogDescription>
              尚未设置输出文件夹,是否打开默认输出文件夹?
              <br />
              <span className="text-xs text-[#8B949E] mt-2 inline-block">{defaultOutputPath}</span>
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setShowOpenFolderDialog(false)}>
              取消
            </Button>
            <Button onClick={openDefaultOutputFolder}>
              <ExternalLink className="w-4 h-4 mr-2" />
              打开
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Settings Dialog */}
      <Dialog open={showSettingsDialog} onOpenChange={setShowSettingsDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>设置</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                {settings.sound_enabled ? (
                  <Volume2 className="w-4 h-4 text-[#00D9FF]" />
                ) : (
                  <VolumeX className="w-4 h-4 text-[#8B949E]" />
                )}
                <Label>完成提示音</Label>
              </div>
              <Switch
                checked={settings.sound_enabled}
                onCheckedChange={(checked) => updateSettings({ sound_enabled: checked })}
              />
            </div>
            <div>
              <Label className="text-xs text-[#8B949E] mb-2 block">并发数</Label>
              {settingsLoaded ? (
                <Select
                  value={String(settings.concurrency)}
                  onValueChange={(v) => {
                    const value = Number(v);
                    if (!isNaN(value) && validateConcurrency(value)) {
                      updateSettings({ concurrency: value });
                    } else {
                      toast.error(`请选择有效的并发数：1, 2, 4, 6, 8`);
                    }
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="选择并发数" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">1</SelectItem>
                    <SelectItem value="2">2</SelectItem>
                    <SelectItem value="4">4</SelectItem>
                    <SelectItem value="6">6</SelectItem>
                    <SelectItem value="8">8</SelectItem>
                  </SelectContent>
                </Select>
              ) : (
                <div className="h-10 w-full rounded-lg border border-[#30363D] bg-[#161B22] px-3 py-2 text-sm text-[#8B949E]">
                  加载中...
                </div>
              )}
            </div>
          </div>
          <DialogFooter>
            <Button onClick={() => setShowSettingsDialog(false)}>关闭</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

export default App;
