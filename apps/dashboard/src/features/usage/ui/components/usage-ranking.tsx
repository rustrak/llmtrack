import { useFormatter, useTranslations } from 'use-intl';
import {
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
} from '@/shared/ui/components/data-table';
import { useMoney } from '@/shared/ui/hooks/use-money';
import {
  avgLatency,
  errorRate,
  type Group,
  type Metric,
  metricOf,
} from '../../model/series';
import { useLatency } from './use-latency';

/**
 * Every group of the report's `group_by` over the range, largest first, with
 * its share of the chart's measure. A row is a way in: `onPick` gets it.
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
  const total = groups.reduce((sum, g) => sum + metricOf(g.totals, metric), 0);

  return (
    <DataTable
      head={
        <>
          <ColumnHead className="min-w-48 capitalize">{groupLabel}</ColumnHead>
          <ColumnHead className="text-right">{t('table.requests')}</ColumnHead>
          <ColumnHead className="text-right">{t('table.errors')}</ColumnHead>
          <ColumnHead className="text-right">{t('table.tokens')}</ColumnHead>
          <ColumnHead className="text-right">{t('table.latency')}</ColumnHead>
          <ColumnHead className="text-right">{t('spend')}</ColumnHead>
          <ColumnHead className="w-48">{t('table.share')}</ColumnHead>
        </>
      }
      empty={groups.length === 0 && <DataTableEmpty title={empty} />}
    >
      {groups.map((group) => {
        const share = total > 0 ? metricOf(group.totals, metric) / total : 0;
        const ms = avgLatency(group.totals);
        return (
          <DataRow
            key={group.key}
            onOpen={onPick ? () => onPick(group) : undefined}
          >
            <Cell className="max-w-72 truncate font-medium">
              {nameOf(group)}
            </Cell>
            <Cell className="text-right tabular-nums">
              {money.compact(group.totals.requests)}
            </Cell>
            <Cell className="text-right tabular-nums text-muted-foreground">
              {format.number(errorRate(group.totals), 'percent')}
            </Cell>
            <Cell className="text-right tabular-nums">
              {money.compact(
                group.totals.prompt_tokens + group.totals.completion_tokens,
              )}
            </Cell>
            <Cell className="text-right tabular-nums text-muted-foreground">
              {ms === null ? '—' : latency(ms)}
            </Cell>
            <Cell className="text-right font-medium tabular-nums">
              {money.usd(group.totals.cost_usd)}
            </Cell>
            <Cell>
              <div className="flex items-center gap-2">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
                  <div
                    className="h-full rounded-full bg-[var(--chart-1)]"
                    style={{ width: `${share * 100}%` }}
                  />
                </div>
                <span className="w-12 text-right text-xs text-muted-foreground tabular-nums">
                  {format.number(share, 'percent')}
                </span>
              </div>
            </Cell>
          </DataRow>
        );
      })}
    </DataTable>
  );
}
