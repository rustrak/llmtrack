import { describe, expect, it } from 'vitest';
import type { FallbackList } from '@/shared/api/schemas';
import {
  fromFallbackRows,
  routerDefaults,
  toFallbackRows,
  toRouterPayload,
} from './router-form';

describe('fallback rows', () => {
  it('round-trip the list of one-key maps', () => {
    const list: FallbackList = [
      { 'gpt-4o': ['claude', 'mini'] },
      { '*': ['mini'] },
    ];
    const rows = toFallbackRows(list);
    expect(rows).toEqual([
      { model: 'gpt-4o', fallbacks: ['claude', 'mini'] },
      { model: '*', fallbacks: ['mini'] },
    ]);
    expect(fromFallbackRows(rows)).toEqual(list);
  });

  it('drop rows with no model or no fallbacks', () => {
    expect(
      fromFallbackRows([
        { model: '', fallbacks: ['x'] },
        { model: 'a', fallbacks: [] },
        { model: 'b', fallbacks: ['c'] },
      ]),
    ).toEqual([{ b: ['c'] }]);
  });
});

describe('router form', () => {
  const settings = {
    num_retries: 2,
    allowed_fails: 3,
    cooldown_time: 5,
    fallbacks: [{ a: ['b'] }],
    context_window_fallbacks: [],
    content_policy_fallbacks: [],
  };

  it('edits numbers as text and sends them back as numbers', () => {
    const values = routerDefaults(settings);
    expect(values.num_retries).toBe('2');
    expect(
      toRouterPayload({ ...values, num_retries: '0', cooldown_time: '30' }),
    ).toEqual({ ...settings, num_retries: 0, cooldown_time: 30 });
  });
});
