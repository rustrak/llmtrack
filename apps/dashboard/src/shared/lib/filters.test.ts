import { describe, expect, it } from 'vitest';
import {
  filterParams,
  filterQuery,
  listParam,
  onlyValue,
  parseList,
  readFilter,
} from './filters';
import { toQuery } from './list-params';

describe('parseList', () => {
  it('reads one value, a comma list or an array', () => {
    expect(parseList(3)).toEqual(['3']);
    expect(parseList('gpt-4o')).toEqual(['gpt-4o']);
    expect(parseList('3, 8,12,8')).toEqual(['3', '8', '12']);
    expect(parseList([3, '8'])).toEqual(['3', '8']);
  });

  it('is empty for nothing', () => {
    expect(parseList(undefined)).toEqual([]);
    expect(parseList('')).toEqual([]);
    expect(parseList(' , ')).toEqual([]);
    expect(parseList({})).toEqual([]);
  });

  it('keeps only ids when asked', () => {
    expect(parseList('3,x,0,-1,2.5,7', true)).toEqual(['3', '7']);
  });
});

describe('listParam', () => {
  it('writes a lone id as a number, the rest as a comma list', () => {
    expect(listParam(['3'])).toBe(3);
    expect(listParam(['gpt-4o'])).toBe('gpt-4o');
    expect(listParam(['3', '8'])).toBe('3,8');
    expect(listParam([])).toBeUndefined();
  });
});

describe('readFilter and filterParams', () => {
  it('reads kept values, else left-out ones', () => {
    expect(readFilter({ team: '3,8' }, 'team', true)).toEqual({
      values: ['3', '8'],
      exclude: false,
    });
    expect(readFilter({ exclude_team: 4 }, 'team', true)).toEqual({
      values: ['4'],
      exclude: true,
    });
    expect(readFilter({ team: 3, exclude_team: 4 }, 'team').values).toEqual([
      '3',
    ]);
    expect(readFilter({}, 'team').values).toEqual([]);
  });

  it('writes one side and clears the other', () => {
    expect(filterParams('key', { values: ['3', '8'], exclude: false })).toEqual(
      { key: '3,8', exclude_key: undefined },
    );
    expect(filterParams('key', { values: ['17'], exclude: true })).toEqual({
      key: undefined,
      exclude_key: 17,
    });
    expect(filterParams('key', { values: [], exclude: true })).toEqual({
      key: undefined,
      exclude_key: undefined,
    });
  });

  it('round-trips', () => {
    const filter = { values: ['a', 'b c'], exclude: true };
    expect(readFilter(filterParams('tag', filter), 'tag')).toEqual(filter);
  });
});

describe('filterQuery', () => {
  it('renames a page’s filters for the API', () => {
    const query = filterQuery(
      { team: 4, exclude_key: '17,18', model: 'gpt' },
      { team: 'team_id', key: 'key_id', person: 'person_id' },
    );
    expect(toQuery(query)).toBe('?team_id=4&exclude_key_id=17%2C18');
  });
});

describe('onlyValue', () => {
  it('is the value when exactly one is kept', () => {
    expect(onlyValue(3)).toBe('3');
    expect(onlyValue('3,8')).toBeUndefined();
    expect(onlyValue(undefined)).toBeUndefined();
  });
});
