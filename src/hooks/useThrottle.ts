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
      if (timeoutRef.current !== null) {
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
        if (timeoutRef.current !== null) {
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
      if (timeoutRef.current !== null) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  return useCallback(
    ((...args: unknown[]) => {
      if (timeoutRef.current !== null) {
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

  // const shouldFlush = () => Date.now() >= lastUpdateTime + throttleInterval;

  let lastFlushedValue: T | null = null;

  const flush = (): T | null => {
    if (flushTimeout !== null) {
      clearTimeout(flushTimeout);
      flushTimeout = null;
    }
    if (pendingValue !== null) {
      lastValue = pendingValue;
      lastFlushedValue = pendingValue;
      lastUpdateTime = Date.now();
      pendingValue = null;
      return lastValue;
    }
    return null;
  };

  const setValue = (newValue: T): T | null => {
    const now = Date.now();

    if (now >= lastUpdateTime + throttleInterval) {
      lastValue = newValue;
      lastUpdateTime = now;
      pendingValue = null;
      lastFlushedValue = newValue;
      return newValue;
    }

    pendingValue = newValue;
    if (!flushTimeout) {
      const remaining = throttleInterval - (now - lastUpdateTime);
      flushTimeout = setTimeout(() => {
        flush();
        flushTimeout = null;
      }, remaining);
    }

    return null;
  };

  const getValue = (): T => lastValue;

  const forceFlush = (): T | null => {
    if (flushTimeout !== null) {
      clearTimeout(flushTimeout);
      flushTimeout = null;
    }
    if (pendingValue !== null) {
      lastValue = pendingValue;
      lastFlushedValue = pendingValue;
      lastUpdateTime = Date.now();
      pendingValue = null;
      return lastValue;
    }
    const result = lastFlushedValue;
    lastFlushedValue = null;
    return result;
  };

  return { setValue, getValue, forceFlush };
}
