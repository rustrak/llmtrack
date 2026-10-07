import { describe, expect, it } from 'vitest';
import {
  nextSort,
  pageRange,
  parseListParams,
  sortOf,
  toQuery,
} from './list-params';

describe('parseListParams', () => {
  it('keeps valid values and drops defaults and junk', () => {
    expect(
      parseListParams({ page: '3', per_page: '25', sort: '-name', q: ' x ' }),
    ).toEqual({ page: 3, per_page: 25, sort: '-name', q: 'x' });
    expect(
      parseListParams({ page: '1', per_page: 50, sort: '', q: '  ' }),
    ).toEqual({});
    expect(parseListParams({ page: '-2', per_page: '9999' })).toEqual({});
  });
});

describe('toQuery', () => {
  it('skips unset values', () => {
    expect(toQuery({ page: 2, q: undefined, sort: '' })).toBe('?page=2');
    expect(toQuery({})).toBe('');
  });
});

describe('sorting', () => {
  it('cycles a header through ascending, descending and the default', () => {
    expect(nextSort(undefined, 'name', '-created_at')).toBe('name');
    expect(nextSort('name', 'name', '-created_at')).toBe('-name');
    expect(nextSort('-name', 'name', '-created_at')).toBeUndefined();
  });

  it('reads the default as the current sort', () => {
    expect(sortOf(undefined, 'created_at', '-created_at')).toBe('desc');
    expect(nextSort(undefined, 'created_at', '-created_at')).toBe('created_at');
    expect(nextSort('created_at', 'created_at', '-created_at')).toBeUndefined();
  });
});

describe('pageRange', () => {
  it('shows the ends and the neighbourhood', () => {
    expect(pageRange(1, 1)).toEqual([1]);
    expect(pageRange(1, 4)).toEqual([1, 2, 3, 4]);
    expect(pageRange(5, 10)).toEqual([1, null, 4, 5, 6, null, 10]);
    expect(pageRange(1, 10)).toEqual([1, 2, null, 10]);
  });
});
