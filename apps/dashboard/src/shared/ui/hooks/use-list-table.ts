import {
  type DataTableColumnDef,
  type DataTableInstance,
  type UseDataTableOptions,
  useDataTable,
} from '@rustrak/ui';
import type { RowData } from '@tanstack/react-table';
import { useMemo, useRef } from 'react';
import type { ListParams } from '@/shared/lib/list-params';
import { toListPatch, toTableQuery } from '@/shared/lib/table-query';

/**
 * The design system's table over one of llmtrack's server-paged lists: the
 * page in hand, its total, and the list's URL params. Sorting a column or
 * moving a page comes back as a URL patch, so the loader fetches it, exactly
 * as before. Column ids are the API's sort names; a column that the server
 * cannot sort says `enableSorting: false`.
 */
export function useListTable<TData extends RowData>({
  data,
  columns,
  total,
  params,
  defaultSort,
  onParams,
  rowMenu,
  ...options
}: {
  data: TData[];
  columns: DataTableColumnDef<TData>[];
  total: number;
  params: ListParams;
  /** The order the server applies when `sort` is unset. */
  defaultSort: string;
  onParams: (patch: Partial<ListParams>) => void;
} & Pick<
  UseDataTableOptions<TData>,
  | 'getRowId'
  | 'rowMenu'
  | 'storageKey'
  | 'initialVisibility'
  | 'enableSelection'
>): DataTableInstance<TData> {
  // One object per URL, not per render: the table takes a new state object
  // as a change, re-renders, and is handed a new one again.
  const { sort, page, per_page } = params;
  const query = useMemo(
    () => toTableQuery({ sort, page, per_page }, defaultSort),
    [sort, page, per_page, defaultSort],
  );
  // The table rebuilds its menu column whenever `rowMenu` changes identity,
  // and remounting Base UI's menus on every render never settles. Pages pass
  // closures, so the table gets one stable function that reads the latest.
  const latestMenu = useRef(rowMenu);
  latestMenu.current = rowMenu;
  const hasMenu = rowMenu !== undefined;
  const stableMenu = useMemo(
    () =>
      hasMenu
        ? (row: Parameters<NonNullable<typeof rowMenu>>[0]) =>
            latestMenu.current?.(row) ?? []
        : undefined,
    [hasMenu],
  );
  return useDataTable({
    data,
    columns,
    rowCount: total,
    query,
    onQueryChange: (updater) => {
      const patch = toListPatch(query, updater(query), defaultSort);
      // The table also proposes what is already so (back to the first page
      // on the first page); navigating to the same URL would render the page
      // again, and the table would propose it again.
      const current = params as Record<string, unknown>;
      const changes = Object.entries(patch).some(
        ([key, value]) => current[key] !== value,
      );
      if (changes) onParams(patch);
    },
    rowMenu: stableMenu,
    ...options,
  });
}
