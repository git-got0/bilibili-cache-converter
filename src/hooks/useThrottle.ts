import { useRef, useCallback, useEffect } from 'react';

/**
 * 节流 Hook - 限制函数调用频率
 * @param value 要节流的值
 * @param interval 节流间隔（毫秒）
 * @returns 当前值（节流后）
 */
export function useThrottle<T>(value: T, interval: number = 150): T {
  const throttledValueRef = useRef<T>(value);
  const lastUpdatedRef = useRef<number>(Date.now());

  useEffect(() => {
    const now = Date.now();
    if (now >= lastUpdatedRef.current + interval) {
      throttledValueRef.current = value;
      lastUpdatedRef.current = now;
    }
  }, [value, interval]);

  return throttledValueRef.current;
}

/**
 * 带回调的节流 Hook - 用于事件处理
 * @param callback 要执行的回调
 * @param delay 延迟时间（毫秒）
 * @returns 可直接调用的节流函数
 */
export function useThrottleCallback<T extends (...args: unknown[]) => void>(
  callback: T,
  delay: number = 150
): T {
  const lastRunRef = useRef<number>(0);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  return useCallback(
    ((...args: unknown[]) => {
      const now = Date.now();
      if (now >= lastRunRef.current + delay) {
        lastRunRef.current = now;
        callback(...args);
      } else {
        if (timeoutRef.current) {
          clearTimeout(timeoutRef.current);
        }
        timeoutRef.current = setTimeout(() => {
          lastRunRef.current = Date.now();
          callback(...args);
        }, delay);
      }
    }) as T,
    [callback, delay]
  );
}

/**
 * 防抖 Hook - 延迟执行函数，直到停止调用一段时间
 * @param callback 要执行的回调
 * @param delay 延迟时间（毫秒）
 * @returns 可直接调用的防抖函数
 */
export function useDebouncedCallback<T extends (...args: unknown[]) => void>(
  callback: T,
  delay: number = 300
): T {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  return useCallback(
    ((...args: unknown[]) => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      timeoutRef.current = setTimeout(() => {
        callback(...args);
      }, delay);
    }) as T,
    [callback, delay]
  );
}

/**
 * 创建可配置节流的进度状态管理
 * 用于处理频繁的进度更新事件
 */
export function createThrottledState<T>(initialValue: T, throttleInterval: number = 150) {
  let lastValue = initialValue;
  let lastUpdateTime = 0;
  let pendingValue: T | null = null;
  let flushTimeout: ReturnType<typeof setTimeout> | null = null;

  const shouldFlush = () => {
    const now = Date.now();
    if (now >= lastUpdateTime + throttleInterval) {
      return true;
    }
    return false;
  };

  const flush = () => {
    if (pendingValue !== null && shouldFlush()) {
      lastValue = pendingValue;
      lastUpdateTime = Date.now();
      pendingValue = null;
      if (flushTimeout) {
        clearTimeout(flushTimeout);
        flushTimeout = null;
      }
      return true;
    }
    return false;
  };

  const setValue = (newValue: T): T | null => {
    const now = Date.now();

    // 如果距离上次更新已经超过节流间隔，立即更新
    if (now >= lastUpdateTime + throttleInterval) {
      lastValue = newValue;
      lastUpdateTime = now;
      pendingValue = null;
      return newValue;
    }

    // 否则缓存新值，安排延迟更新
    pendingValue = newValue;
    if (!flushTimeout) {
      const remaining = throttleInterval - (now - lastUpdateTime);
      flushTimeout = setTimeout(() => {
        flush();
        flushTimeout = null;
      }, remaining);
    }

    return null; // 返回 null 表示值被缓存，尚未应用
  };

  const getValue = (): T => lastValue;

  const forceFlush = (): T | null => {
    if (flushTimeout) {
      clearTimeout(flushTimeout);
      flushTimeout = null;
    }
    if (flush()) {
      return lastValue;
    }
    return pendingValue;
  };

  return { setValue, getValue, forceFlush };
}
