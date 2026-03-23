import { describe, it, expect, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useVirtualList } from '@/hooks/useVirtualList';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

describe('useVirtualList', () => {
  const mockItems = Array.from({ length: 100 }, (_, i) => ({
    id: `file-${i}`,
    name: `test-file-${i}.mp4`,
    path: `/test/path/test-file-${i}.mp4`,
  }));

  it('should initialize with empty items', () => {
    const { result } = renderHook(() =>
      useVirtualList({
        items: [],

        itemHeight: 40,
      })
    );

    expect(result.current.virtualItems.length).toBe(0);
    expect(result.current.totalSize).toBe(0);
  });

  it('should calculate total size correctly', () => {
    const { result } = renderHook(() =>
      useVirtualList({
        items: mockItems.slice(0, 10),

        itemHeight: 40,
      })
    );

    expect(result.current.totalSize).toBe(400); // 10 items * 40px
  });

  it('should handle items correctly', () => {
    const { result } = renderHook(() =>
      useVirtualList({
        items: mockItems,

        itemHeight: 36,
        overscan: 5,
      })
    );
    expect(result.current.totalSize).toBe(100 * 36);
  });

  it('should scroll to index', () => {
    const { result } = renderHook(() =>
      useVirtualList({
        items: Array.from({ length: 100 }, (_, i) => ({
          id: `file-${i}`,
          name: `test-file-${i}.mp4`,
          path: `/test/path/test-file-${i}.mp4`,
        })),

        itemHeight: 40,
      })
    );

    // Scroll to index should not throw
    expect(() => {
      act(() => {
        result.current.scrollToIndex(50);
      });
    }).not.toThrow();
  });

  it('should scroll to top', () => {
    const { result } = renderHook(() =>
      useVirtualList({
        items: Array.from({ length: 100 }, (_, i) => ({
          id: `file-${i}`,
          name: `test-file-${i}.mp4`,
          path: `/test/path/test-file-${i}.mp4`,
        })),

        itemHeight: 40,
      })
    );

    expect(() => {
      act(() => {
        result.current.scrollToTop();
      });
    }).not.toThrow();
  });

  it('should scroll to bottom', () => {
    const { result } = renderHook(() =>
      useVirtualList({
        items: Array.from({ length: 100 }, (_, i) => ({
          id: `file-${i}`,
          name: `test-file-${i}.mp4`,
          path: `/test/path/test-file-${i}.mp4`,
        })),

        itemHeight: 40,
      })
    );

    expect(() => {
      act(() => {
        result.current.scrollToBottom();
      });
    }).not.toThrow();
  });
});
