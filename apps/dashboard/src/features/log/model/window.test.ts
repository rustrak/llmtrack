import { describe, expect, it } from 'vitest';
import { LOG_WINDOWS, parseWindow, since } from './window';

const now = new Date('2026-10-07T12:00:00Z');

describe('log windows', () => {
  it('start that long before now', () => {
    expect(since('1h', now)).toBe('2026-10-07T11:00:00.000Z');
    expect(since('24h', now)).toBe('2026-10-06T12:00:00.000Z');
    expect(since('7d', now)).toBe('2026-09-30T12:00:00.000Z');
    expect(since('all', now)).toBeUndefined();
  });

  it('default to the last 24 hours', () => {
    expect(parseWindow(undefined)).toBe('24h');
    expect(parseWindow('nonsense')).toBe('24h');
    for (const window of LOG_WINDOWS) expect(parseWindow(window)).toBe(window);
  });
});
