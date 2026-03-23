import React, { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

/**
 * 错误边界组件
 * 用于捕获子组件树中的 JavaScript 错误，
 * 防止整个应用崩溃显示白屏
 */
export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    // 可以在这里上报错误到服务器
  }

  handleReload = () => {
    window.location.reload();
  };

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      // 如果有自定义 fallback，则使用自定义内容
      if (this.props.fallback) {
        return this.props.fallback;
      }

      // 默认错误显示
      return (
        <div className="h-screen flex items-center justify-center bg-gradient-to-br from-[#0D1117] via-[#161B22] to-[#0D1117] text-white p-4">
          <div className="text-center max-w-md">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-[#EF4444]/20 flex items-center justify-center">
              <svg
                className="w-8 h-8 text-[#EF4444]"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                />
              </svg>
            </div>
            <h1 className="text-xl font-bold mb-2">页面加载失败</h1>
            <p className="text-[#8B949E] text-sm mb-4">应用遇到了未知错误，请尝试重新加载</p>
            {this.state.error && (
              <div className="bg-[#21262D] rounded-lg p-3 mb-4 text-left">
                <p className="text-[#EF4444] text-xs font-mono break-all">
                  {this.state.error.message}
                </p>
              </div>
            )}
            <div className="flex gap-3 justify-center">
              <button
                onClick={this.handleReset}
                className="px-4 py-2 bg-[#21262D] hover:bg-[#30363D] rounded-lg text-sm transition-colors"
              >
                重试
              </button>
              <button
                onClick={this.handleReload}
                className="px-4 py-2 bg-gradient-to-r from-[#00D9FF] to-[#8B5CF6] hover:opacity-90 rounded-lg text-sm font-medium transition-opacity"
              >
                重新加载
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
