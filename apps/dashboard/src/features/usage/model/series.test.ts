import { describe, expect, it } from 'vitest';
import type { Totals } from '@/shared/api/schemas';
import {
  availableGroups,
  avgLatency,
  bucketOf,
  bucketTotals,
  defaultGranularity,
  delta,
  effectiveGroup,
  emptyTotals,
  narrowTo,
  OTHER,
  stack,
  tokenParts,
} from './series';

const totals = (patch: Partial<Totals>): Totals => ({
  ...emptyTotals(),
  ...patch,
});
const day = (d: string, patch: Partial<Totals> = {}) => ({
  day: d,
  ...totals(patch),
});

describe('buckets', () => {
  it('maps a day to itself, its Monday or its month', () => {
    expect(bucketOf('2026-10-07', 'day')).toBe('2026-10-07');
    expect(bucketOf('2026-10-07', 'week')).toBe('2026-10-05');
    expect(bucketOf('2026-10-05', 'week')).toBe('2026-10-05');
    expect(bucketOf('2026-10-11', 'week')).toBe('2026-10-05');
    expect(bucketOf('2026-10-07', 'month')).toBe('2026-10-01');
  });

  it('picks a granularity from the range length', () => {
    expect(defaultGranularity('2026-10-01', '2026-10-31')).toBe('day');
    expect(defaultGranularity('2026-07-01', '2026-10-01')).toBe('week');
    expect(defaultGranularity('2026-01-01', '2026-10-01')).toBe('month');
  });

  it('sums days into their buckets, empty ones kept', () => {
    const rows = bucketTotals(
      [
        day('2026-09-30', { requests: 1 }),
        day('2026-10-01', { requests: 2 }),
        day('2026-10-02'),
      ],
      'month',
    );
    expect(rows.map((r) => [r.bucket, r.totals.requests])).toEqual([
      ['2026-09-01', 1],
      ['2026-10-01', 2],
    ]);
  });
});

describe('stack', () => {
  const point = (d: string, key: string, cost: number) => ({
    ...day(d, { cost_usd: cost }),
    key,
    label: key,
  });

  it('keeps the top groups and folds the rest into Other', () => {
    const series = ['a', 'b', 'c', 'd', 'e', 'f', 'g'].map((key, i) =>
      point('2026-10-01', key, 10 - i),
    );
    const { keys, hasOther, rows } = stack(
      { daily: [day('2026-10-01')], series },
      'spend',
      'day',
    );
    expect(keys.map((k) => k.key)).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(hasOther).toBe(true);
    expect(rows[0][OTHER]).toBe(5 + 4);
    expect(rows[0].a).toBe(10);
  });

  it('orders the kept groups by name, so colours follow the group', () => {
    const { keys } = stack(
      {
        daily: [day('2026-10-01')],
        series: [
          point('2026-10-01', 'zeta', 9),
          point('2026-10-01', 'alpha', 1),
        ],
      },
      'spend',
      'day',
    );
    expect(keys.map((k) => k.key)).toEqual(['alpha', 'zeta']);
  });
});

describe('figures', () => {
  it('compares with the previous period', () => {
    expect(delta(12, 10)).toBeCloseTo(0.2);
    expect(delta(5, 0)).toBeNull();
  });

  it('averages latency over requests', () => {
    expect(avgLatency(totals({ requests: 4, latency_ms: 1000 }))).toBe(250);
    expect(avgLatency(totals({}))).toBeNull();
  });

  it('splits tokens into parts that add up', () => {
    expect(
      tokenParts(
        totals({
          prompt_tokens: 100,
          cached_tokens: 30,
          completion_tokens: 50,
          reasoning_tokens: 20,
        }),
      ),
    ).toEqual({ input: 70, cached: 30, output: 30, reasoning: 20 });
  });
});

describe('groupings', () => {
  it('drops what the filters already pin down', () => {
    expect(availableGroups({})).toEqual([
      'model',
      'team',
      'key',
      'end_user',
      'tag',
    ]);
    expect(availableGroups({ team: 1 })).toEqual([
      'model',
      'key',
      'person',
      'end_user',
      'tag',
    ]);
    expect(availableGroups({ team: 1, person: 3 })).toEqual(['model', 'key']);
    expect(availableGroups({ key: 2 })).toEqual(['model', 'end_user', 'tag']);
    expect(availableGroups({ model: 'gpt' })).toEqual(['team', 'key']);
    expect(availableGroups({ key: 2, model: 'gpt' })).toEqual([]);
  });

  it('falls back when the wanted grouping no longer applies', () => {
    expect(effectiveGroup('team', { team: 1 })).toBe('model');
    expect(effectiveGroup('tag', {})).toBe('tag');
    expect(effectiveGroup(undefined, { model: 'gpt' })).toBe('team');
  });
});

describe('narrowTo', () => {
  it('filters by what the page can, opens the logs for the rest', () => {
    expect(narrowTo('model', 'gpt-4o')).toEqual({
      filter: { model: 'gpt-4o' },
    });
    expect(narrowTo('team', '3')).toEqual({ filter: { team: 3 } });
    expect(narrowTo('person', '7')).toEqual({ filter: { person: 7 } });
    expect(narrowTo('tag', 'prod')).toEqual({ logs: { tag: 'prod' } });
  });

  it('has nothing for personal keys or keys of no one', () => {
    expect(narrowTo('team', 'user:5')).toBeNull();
    expect(narrowTo('person', '')).toBeNull();
  });
});
