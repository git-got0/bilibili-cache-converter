import { useRef, useMemo, useCallback, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

/**
 * 虚拟列表配置选项
 */
export interface VirtualListOptions<T> {
  /** 数据列表 */
  items: T[];
  /** 单项高度（固定高度模式）*/
  itemHeight?: number;
  /** 预估高度（动态高度模式） */
  estimatedItemSize?: number;
  /** 溢出缓冲项数 */
  overscan?: number;
  /** 获取 key */
  getKey?: (item: T, index: number) => string | number;
}

/**
 * 虚拟项类型
 */
export interface VirtualItem {
  index: number;
  key: string | number;
  size: number;
  start: number;
}

/**
 * 虚拟列表返回值
 */
export interface VirtualListResult {
  /** 虚拟列表容器 ref */
  virtualizer: ReturnType<typeof useVirtualizer>;
  /** 虚拟项列表（用于渲染） */
  virtualItems: VirtualItem[];
  /** 总尺寸 */
  totalSize: number;
  /** 滚动到指定索引 */
  scrollToIndex: (index: number) => void;
  /** 滚动到顶部 */
  scrollToTop: () => void;
  /** 滚动到底部 */
  scrollToBottom: () => void;
  /** 容器 ref */
  parentRef: React.RefObject<HTMLDivElement>;
}

/**
 * 虚拟列表 Hook - 封装 @tanstack/react-virtual
 * 用于优化大量数据的渲染性能
 */
export function useVirtualList<T>({
  items,
  itemHeight = 40,
  estimatedItemSize,
  overscan = 5,
  getKey,
}: VirtualListOptions<T>): VirtualListResult {
  const parentRef = useRef<HTMLDivElement | null>(null);

  const keyGetter = useCallback(
    (index: number) => {
      if (getKey) {
        return getKey(items[index], index);
      }
      const item = items[index];
      if (item && typeof item === 'object' && 'id' in item) {
        return (item as { id: string | number }).id;
      }
      return index;
    },
    [items, getKey]
  );
  // 关键修复：添加元素尺寸观察，确保虚拟列表正确初始化
  // @tanstack/react-v3 需要显式配置或使用 undefined 启用默认行为
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => {
      if (!parentRef.current) {
        console.warn('[useVirtualList] parentRef.current is null');
        return null;
      }
      return parentRef.current as Element | null;
    },
    estimateSize: useCallback(() => {
      const size = estimatedItemSize || itemHeight;
      if (size <= 0) {
        console.warn('[useVirtualList] estimateSize is zero or negative:', size);
      }
      return size;
    }, [estimatedItemSize, itemHeight]),
    overscan,
    getItemKey: keyGetter,
    // 关键修复：确保在 items 变化时重新测量
    measureElement: (element: Element) => {
      if (!(element instanceof HTMLElement)) return itemHeight;
      return element.offsetHeight || itemHeight;
    },
    // 初始偏移量，帮助库在挂载前计算
    initialRect: { width: 0, height: 200 },
  });

  // Debug: Log virtualizer initialization
  useMemo(() => {
    if (items.length > 0) {
      const vItems = virtualizer.getVirtualItems();
      console.log(
        '[useVirtualList] ✅ items:',
        items.length,
        '| virtualItems:',
        vItems.length,
        '| totalSize:',
        virtualizer.getTotalSize()
      );
    }
  }, [items.length, virtualizer]);

  // 关键修复：当 items 变化时，强制重新计算虚拟列表
  useEffect(() => {
    if (items.length > 0 && parentRef.current) {
      console.log('[useVirtualList] Items changed, remeasuring...');
      // 触发重新测量
      virtualizer.measure();
    }
  }, [items.length, virtualizer]);

  const scrollToIndex = useCallback(
    (index: number) => {
      virtualizer.scrollToIndex(index, { align: 'start' });
    },
    [virtualizer]
  );

  const scrollToTop = useCallback(() => {
    const element = parentRef.current;
    if (element && 'scrollTop' in element) {
      (element as HTMLElement).scrollTop = 0;
    }
  }, []);

  const scrollToBottom = useCallback(() => {
    const element = parentRef.current;
    if (element && 'scrollHeight' in element && 'scrollTop' in element) {
      const el = element as HTMLElement;
      el.scrollTop = el.scrollHeight;
    }
  }, []);

  return {
    virtualizer,
    virtualItems: virtualizer.getVirtualItems() as VirtualItem[],
    totalSize: virtualizer.getTotalSize(),
    scrollToIndex,
    scrollToTop,
    scrollToBottom,
    parentRef: parentRef as React.RefObject<HTMLDivElement>,
  };
}

/**
 * 固定高度虚拟列表 - 简化版，适用于所有项高度相同的场景
 */
export function useFixedVirtualList<T>(
  items: T[],
  itemHeight: number = 40,
  overscan: number = 5
): VirtualListResult {
  return useVirtualList<T>({
    items,
    itemHeight,
    overscan,
  });
}
