import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import AppComponent from './App.tsx';
import './index.css';
import { Toaster } from 'sonner';
import { ErrorBoundary } from './components/ErrorBoundary';

function LoadingScreen() {
  return (
    <div
      style={{
        width: '100vw',
        height: '100vh',
        background: 'linear-gradient(135deg, #0D1117 0%, #161B22 100%)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
    >
      <div className="text-center">
        <div className="w-8 h-8 border-2 border-[#00D9FF] border-t-transparent rounded-full animate-spin mx-auto mb-4"></div>
        <p className="text-[#8B949E]">加载中...</p>
      </div>
    </div>
  );
}
function App() {
  const [isReady, setIsReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadingStep, setLoadingStep] = useState('初始化...');

  useEffect(() => {
    const init = async () => {
      try {
        // 步骤 1: 检查 Tauri API
        setLoadingStep('检查环境...');
        await new Promise((resolve) => setTimeout(resolve, 100));

        // 步骤 2: 加载设置
        setLoadingStep('加载设置...');
        // await invoke('get_settings');  // 如果有这个调用

        // 步骤 3: 完成
        setLoadingStep('准备就绪...');
        await new Promise((resolve) => setTimeout(resolve, 100));

        setIsReady(true);
      } catch (err) {
        console.error('Init error:', err);
        setError(
          `初始化失败：${String(err)}\n\n请检查:\n1. 日志目录权限\n2. FFmpeg 是否安装\n3. 系统兼容性`
        );
      }
    };

    init();

    // 添加超时保护（延长到 60 秒，避免大文件夹扫描误报）
    // 已禁用：存在闭包问题，会导致程序正常运行 60 秒后误报错误
    /*
    const timeoutId = setTimeout(() => {
      if (!isReady) {
        setError(
          '加载超时（超过 60 秒）\n\n可能原因:\n• 日志系统阻塞\n• 文件系统访问缓慢\n• 杀毒软件拦截'
        );
      }
    }, 60000); // 60 秒

    return () => clearTimeout(timeoutId);
    */
  }, []);

  if (error) {
    return (
      <div
        style={{
          width: '100vw',
          height: '100vh',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          background: '#0D1117',
          color: '#EF4444',
          padding: '40px',
        }}
      >
        <div style={{ maxWidth: '600px' }}>
          <h2>❌ 启动失败</h2>
          <pre
            style={{
              background: '#161B22',
              padding: '20px',
              borderRadius: '8px',
              whiteSpace: 'pre-wrap',
              fontSize: '12px',
            }}
          >
            {error}
          </pre>
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: '20px',
              padding: '10px 20px',
              background: '#00D9FF',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer',
            }}
          >
            重试
          </button>
        </div>
      </div>
    );
  }
  if (!isReady) {
    return <LoadingScreen />;
  }

  return (
    <StrictMode>
      <ErrorBoundary>
        <AppComponent />
        <Toaster position="bottom-right" richColors />
      </ErrorBoundary>
    </StrictMode>
  );
}

createRoot(document.getElementById('root')!).render(<App />);
