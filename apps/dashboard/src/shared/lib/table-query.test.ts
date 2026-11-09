import { describe, expect, it } from 'vitest';
import { barFilters, barPatch, toListPatch, toTableQuery } from './table-query';

describe('table query', () => {
  const base = toTableQuery({ page: 3, per_page: 25 }, '-created_at');

  it('reads the URL, showing the default sort', () => {
    expect(base.sorting).toEqual([{ id: 'created_at', desc: true }]);
    expect(base.pagination).toEqual({ pageIndex: 2, pageSize: 25 });
  });

  it('a new sort goes back to the first page, the default leaves the URL', () => {
    const next = { ...base, sorting: [{ id: 'name', desc: false }] };
    expect(toListPatch(base, next, '-created_at')).toEqual({
      sort: 'name',
      page: undefined,
    });
    const back = toTableQuery({ sort: 'name' }, '-created_at');
    expect(toListPatch(back, { ...back, sorting: [] }, '-created_at')).toEqual({
      sort: undefined,
      page: undefined,
    });
  });

  it('pages and page sizes', () => {
    const page = { ...base, pagination: { pageIndex: 0, pageSize: 25 } };
    expect(toListPatch(base, page, '-created_at')).toEqual({ page: undefined });
    const size = { ...base, pagination: { pageIndex: 2, pageSize: 50 } };
    expect(toListPatch(base, size, '-created_at')).toEqual({
      per_page: undefined,
      page: undefined,
    });
  });

  it('the bar reads and writes the list params by name', () => {
    const keys = ['team_id', 'exclude_team_id', 'status'];
    expect(barFilters(keys, { team_id: 3, exclude_team_id: '4,5' })).toEqual([
      { id: 'team_id', value: ['3'] },
      { id: 'exclude_team_id', value: ['4', '5'] },
    ]);
    expect(
      barPatch(keys, {
        filters: [{ id: 'team_id', value: ['8'] }],
        search: ' prod ',
      }),
    ).toEqual({
      q: 'prod',
      page: undefined,
      team_id: 8,
      exclude_team_id: undefined,
      status: undefined,
    });
  });
});
