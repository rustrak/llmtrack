import { createDataTableColumnHelper, DataTable } from '@rustrak/ui';
import { useMemo, useState } from 'react';
import { useFormatter, useTranslations } from 'use-intl';
import { DEFAULT_PAGE_SIZE, type ListParams } from '@/shared/lib/list-params';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';
import {
  avgLatency,
  errorRate,
  type Group,
  type Metric,
  metricOf,
} from '../../model/series';
import { useLatency } from './use-latency';

const column = createDataTableColumnHelper<Group>();
const num = 'font-mono text-mono-sm tabular-nums';

/**
 * Every group of the report's `group_by` over the range, largest first, with
 * its share of the chart's measure. A row is a way in: `onPick` gets it.
 * The groups come whole, so the table pages them here and keeps their rank.
 */
export function UsageRanking({
  groups,
  metric,
  groupLabel,
  nameOf,
  onPick,
  empty,
}: {
  groups: Group[];
  metric: Metric;
  groupLabel: string;
  nameOf: (group: Group) => string;
  onPick?: (group: Group) => void;
  empty: string;
}) {
  const t = useTranslations('usage');
  const money = useMoney();
  const format = useFormatter();
  const latency = useLatency();
  const [params, setParams] = useState<ListParams>({});
  const total = groups.reduce((sum, g) => sum + metricOf(g.totals, metric), 0);
  const size = params.per_page ?? DEFAULT_PAGE_SIZE;
  const start = ((params.page ?? 1) - 1) * size;

  const columns = useMemo(
    () =>
      column.columns([
        column.display({
          id: 'name',
          header: groupLabel,
          enableSorting: false,
          enableHiding: false,
          cell: ({ row }) => (
            <span className="truncate text-fg text-value">
              {nameOf(row.original)}
            </span>
          ),
          meta: { label: groupLabel },
        }),
        column.display({
          id: 'requests',
          header: t('table.requests'),
          enableSorting: false,
          cell: ({ row }) => (
            <span className={num}>
              {money.compact(row.original.totals.requests)}
            </span>
          ),
          meta: {
            label: t('table.requests'),
            width: 112,
            align: 'end',
            numeric: true,
          },
        }),
        column.display({
          id: 'errors',
          header: t('table.errors'),
          enableSorting: false,
          cell: ({ row }) => (
            <span className={`${num} text-fg-subtle`}>
              {format.number(errorRate(row.original.totals), 'percent')}
            </span>
          ),
          meta: {
            label: t('table.errors'),
            width: 96,
            align: 'end',
            numeric: true,
          },
        }),
        column.display({
          id: 'tokens',
          header: t('table.tokens'),
          enableSorting: false,
          cell: ({ row }) => {
            const x = row.original.totals;
            return (
              <span className={num}>
                {money.compact(x.prompt_tokens + x.completion_tokens)}
              </span>
            );
          },
          meta: {
            label: t('table.tokens'),
            width: 112,
            align: 'end',
            numeric: true,
          },
        }),
        column.display({
          id: 'latency',
          header: t('table.latency'),
          enableSorting: false,
          cell: ({ row }) => {
            const ms = avgLatency(row.original.totals);
            return (
              <span className={`${num} text-fg-subtle`}>
                {ms === null ? '—' : latency(ms)}
              </span>
            );
          },
          meta: {
            label: t('table.latency'),
            width: 112,
            align: 'end',
            numeric: true,
          },
        }),
        column.display({
          id: 'spend',
          header: t('spend'),
          enableSorting: false,
          cell: ({ row }) => (
            <span className={`${num} text-fg`}>
              {money.usd(row.original.totals.cost_usd)}
            </span>
          ),
          meta: { label: t('spend'), width: 120, align: 'end', numeric: true },
        }),
        column.display({
          id: 'share',
          header: t('table.share'),
          enableSorting: false,
          cell: ({ row }) => {
            const share =
              total > 0 ? metricOf(row.original.totals, metric) / total : 0;
            return (
              <div className="flex w-full items-center gap-2">
                <div className="h-1 flex-1 overflow-hidden rounded-pill bg-surface-chip">
                  <div
                    className="h-full rounded-pill bg-chart-1"
                    style={{ width: `${share * 100}%` }}
                  />
                </div>
                <span className={`w-12 text-right text-fg-subtle ${num}`}>
                  {format.number(share, 'percent')}
                </span>
              </div>
            );
          },
          meta: { label: t('table.share'), width: 192 },
        }),
      ]),
    [t, money, format, latency, groupLabel, nameOf, metric, total],
  );

  const table = useListTable({
    data: groups.slice(start, start + size),
    columns,
    total: groups.length,
    params,
    // Nothing sorts: the groups keep their rank.
    defaultSort: 'rank',
    onParams: (patch) => setParams((prev) => ({ ...prev, ...patch })),
    getRowId: (group) => group.key,
    storageKey: 'usage-ranking',
  });

  return (
    <DataTable
      table={table}
      className="min-h-0 flex-1"
      empty={{ title: empty }}
      onRowClick={onPick ? (row) => onPick(row.original) : undefined}
    />
  );
}
