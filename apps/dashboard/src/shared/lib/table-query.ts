import type { DataTableQuery } from '@rustrak/ui';
import { type ListParam, listParam, parseList } from './filters';
import { DEFAULT_PAGE_SIZE, type ListParams } from './list-params';

/**
 * The seam between llmtrack's list URLs (`?sort=-spend&page=2&per_page=25`,
 * the same names the API takes) and the design system's table state. Filters
 * and the search stay in the page's toolbar, so only sorting and paging cross.
 */

/** `-spend` as the table's sorting: one column, descending. */
export function toSorting(sort: string): DataTableQuery['sorting'] {
  return sort.startsWith('-')
    ? [{ id: sort.slice(1), desc: true }]
    : [{ id: sort, desc: false }];
}

/** The table's state for a list's URL; an unset sort shows the default. */
export function toTableQuery(
  params: ListParams,
  defaultSort: string,
): DataTableQuery {
  return {
    sorting: toSorting(params.sort ?? defaultSort),
    filters: [],
    search: '',
    pagination: {
      pageIndex: (params.page ?? 1) - 1,
      pageSize: params.per_page ?? DEFAULT_PAGE_SIZE,
    },
  };
}

/**
 * What changed, as a URL patch. A new sort or page size starts again from the
 * first page; defaults are left out so a fresh list is a clean address.
 */
export function toListPatch(
  previous: DataTableQuery,
  next: DataTableQuery,
  defaultSort: string,
): Partial<ListParams> {
  const [first] = next.sorting;
  const sort = first ? `${first.desc ? '-' : ''}${first.id}` : defaultSort;
  const before = previous.sorting[0];
  const sortBefore = before
    ? `${before.desc ? '-' : ''}${before.id}`
    : defaultSort;
  const size = next.pagination.pageSize;

  if (sort !== sortBefore) {
    return { sort: sort === defaultSort ? undefined : sort, page: undefined };
  }
  if (size !== previous.pagination.pageSize) {
    return {
      per_page: size === DEFAULT_PAGE_SIZE ? undefined : size,
      page: undefined,
    };
  }
  const page = next.pagination.pageIndex + 1;
  return { page: page > 1 ? page : undefined };
}

/**
 * The query bar's side. A field's key is the URL param it writes (`team_id`,
 * `exclude_team_id`, `status`), its value the param's list, and the free text
 * is `q`. So the bar reads the page's search as it is and hands back a patch
 * of the same names: the loader and the API do not change.
 */
export function barFilters(
  keys: readonly string[],
  search: object,
): DataTableQuery['filters'] {
  const params = search as Record<string, unknown>;
  return keys.flatMap((key) => {
    const values = parseList(params[key]);
    return values.length > 0 ? [{ id: key, value: values }] : [];
  });
}

/** The URL patch for what the bar now holds: every field, set or cleared. */
export function barPatch(
  keys: readonly string[],
  next: { filters: DataTableQuery['filters']; search: string },
): Record<string, ListParam | undefined> {
  const patch: Record<string, ListParam | undefined> = {
    q: next.search.trim() || undefined,
    page: undefined,
  };
  for (const key of keys) {
    const value = next.filters.find((f) => f.id === key)?.value;
    patch[key] = listParam(
      Array.isArray(value) ? value.map(String) : value ? [String(value)] : [],
    );
  }
  return patch;
}
