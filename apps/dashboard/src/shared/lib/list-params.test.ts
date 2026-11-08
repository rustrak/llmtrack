import { describe, expect, it } from 'vitest';
import { parseListParams, toQuery } from './list-params';

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
