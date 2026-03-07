import { describe, it, expect } from 'vitest';
import { formatFileSize, formatTime, cn } from '@/lib/utils';

describe('formatFileSize', () => {
  it('should return 0 B for 0 bytes', () => {
    expect(formatFileSize(0)).toBe('0 B');
  });

  it('should format bytes correctly', () => {
    expect(formatFileSize(512)).toBe('512 B');
    expect(formatFileSize(1024)).toBe('1 KB');
    expect(formatFileSize(1536)).toBe('1.5 KB');
  });

  it('should format megabytes correctly', () => {
    expect(formatFileSize(1024 * 1024)).toBe('1 MB');
    expect(formatFileSize(1024 * 1024 * 5)).toBe('5 MB');
    expect(formatFileSize(1024 * 1024 * 1.5)).toBe('1.5 MB');
  });

  it('should format gigabytes correctly', () => {
    expect(formatFileSize(1024 * 1024 * 1024)).toBe('1 GB');
    expect(formatFileSize(1024 * 1024 * 1024 * 2)).toBe('2 GB');
  });

  it('should format terabytes correctly', () => {
    expect(formatFileSize(1024 * 1024 * 1024 * 1024)).toBe('1 TB');
  });
});

describe('formatTime', () => {
  it('should return 0秒 for 0 seconds', () => {
    expect(formatTime(0)).toBe('0秒');
  });

  it('should format seconds correctly', () => {
    expect(formatTime(30)).toBe('30秒');
    expect(formatTime(59)).toBe('59秒');
  });

  it('should format minutes correctly', () => {
    expect(formatTime(60)).toBe('1分钟');
    expect(formatTime(90)).toBe('1分钟30秒');
    expect(formatTime(120)).toBe('2分钟');
  });

  it('should format hours correctly', () => {
    expect(formatTime(3600)).toBe('1小时');
    expect(formatTime(3660)).toBe('1小时1分钟');
    expect(formatTime(3661)).toBe('1小时1分钟1秒');
    expect(formatTime(7200)).toBe('2小时');
  });

  it('should format complex time correctly', () => {
    expect(formatTime(86399)).toBe('23小时59分钟59秒');
  });
});

describe('cn', () => {
  it('should merge class names', () => {
    expect(cn('foo', 'bar')).toBe('foo bar');
  });

  it('should handle conditional classes', () => {
    const condition = true;
    expect(cn('foo', condition && 'bar')).toBe('foo bar');
    expect(cn('foo', condition && '')).toBe('foo');
  });

  it('should handle empty classes', () => {
    expect(cn('', 'bar')).toBe('bar');
    expect(cn()).toBe('');
  });
});
