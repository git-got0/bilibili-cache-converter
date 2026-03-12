import { useRef, useMemo, useCallback } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

/**
 * 虚拟列表配置选项
 */
export interface VirtualListOptions<T> {
  /** 数据列表 */
  items: T[];
  /** 容器高度 */
  containerHeight: number;
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
export interface VirtualListResult<T> {
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
  containerHeight,
  itemHeight = 40,
  estimatedItemSize,
  overscan = 5,
  getKey,
}: VirtualListOptions<T>): VirtualListResult<T> {
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

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current as Element | null,
    estimateSize: useCallback(() => estimatedItemSize || itemHeight, [estimatedItemSize, itemHeight]),
    overscan,
    getItemKey: keyGetter,
  });

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
  containerHeight: number = 300,
  overscan: number = 5
): VirtualListResult<T> {
  return useVirtualList<T>({
    items,
    itemHeight,
    containerHeight,
    overscan,
  });
}
