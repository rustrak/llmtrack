import { describe, expect, it } from 'vitest';
import { rangeFor, resolveRange } from './range';

const today = new Date('2026-10-07T23:30:00Z');

describe('rangeFor', () => {
  it('counts today as the last day of the range', () => {
    expect(rangeFor('7d', today)).toEqual({
      from: '2026-10-01',
      to: '2026-10-07',
    });
    expect(rangeFor('30d', today)).toEqual({
      from: '2026-09-08',
      to: '2026-10-07',
    });
  });

  it('starts this month on the first', () => {
    expect(rangeFor('month', today)).toEqual({
      from: '2026-10-01',
      to: '2026-10-07',
    });
  });

  it('covers the whole of last month', () => {
    expect(rangeFor('last-month', today)).toEqual({
      from: '2026-09-01',
      to: '2026-09-30',
    });
  });
});

describe('resolveRange', () => {
  it('takes a custom range when both days are valid and in order', () => {
    expect(
      resolveRange({ from: '2026-09-01', to: '2026-09-10' }, today),
    ).toEqual({ from: '2026-09-01', to: '2026-09-10' });
  });

  it('falls back to the preset, or 30 days, otherwise', () => {
    expect(
      resolveRange({ from: '2026-09-10', to: '2026-09-01' }, today),
    ).toEqual(rangeFor('30d', today));
    expect(resolveRange({ from: 'nope', to: '2026-09-01' }, today)).toEqual(
      rangeFor('30d', today),
    );
    expect(resolveRange({ range: '7d' }, today)).toEqual(rangeFor('7d', today));
  });
});
