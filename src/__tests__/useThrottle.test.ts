import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createThrottledState } from '@/hooks/useThrottle';

describe('createThrottledState', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('should return initial value immediately', () => {
    const { getValue } = createThrottledState(0, 100);
    expect(getValue()).toBe(0);
  });

  it('should apply value immediately if enough time has passed', () => {
    const { setValue, getValue } = createThrottledState(0, 100);

    vi.advanceTimersByTime(200);

    const result = setValue(1);
    expect(result).toBe(1);
    expect(getValue()).toBe(1);
  });

  it('should throttle rapid updates', () => {
    const { setValue, getValue } = createThrottledState(0, 100);

    // First call should apply immediately
    const result1 = setValue(1);
    expect(result1).toBe(1);

    // Subsequent calls within throttle period should be cached
    const result2 = setValue(2);
    expect(result2).toBeNull();

    // Value should still be the first value
    expect(getValue()).toBe(1);
  });

  it('should flush cached value after throttle period', () => {
    const { setValue, getValue, forceFlush } = createThrottledState(0, 100);

    // First call
    setValue(1);

    // Second call (should be cached)
    setValue(2);

    // Advance time past throttle period
    vi.advanceTimersByTime(150);

    // Force flush should now apply cached value
    const result = forceFlush();
    expect(result).toBe(2);
    expect(getValue()).toBe(2);
  });

  it('should allow forceFlush to be called multiple times', () => {
    const { setValue, forceFlush } = createThrottledState(0, 100);

    setValue(1);
    vi.advanceTimersByTime(150);

    const result1 = forceFlush();
    expect(result1).toBe(1);

    // Calling again should return null since no new value
    const result2 = forceFlush();
    expect(result2).toBeNull();
  });

  it('should handle rapid consecutive updates', () => {
    const { setValue, getValue, forceFlush } = createThrottledState(0, 50);

    setValue(1);
    setValue(2);
    setValue(3);
    setValue(4);
    setValue(5);

    vi.advanceTimersByTime(100);

    const result = forceFlush();
    expect(result).toBe(5);
    expect(getValue()).toBe(5);
  });
});
