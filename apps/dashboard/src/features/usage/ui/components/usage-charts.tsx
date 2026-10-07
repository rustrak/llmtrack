import { ArrowDownRight, ArrowUpRight } from 'lucide-react';
import type { ReactNode } from 'react';
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { useFormatter, useTranslations } from 'use-intl';
import { cn } from '@/shared/lib/utils';
import type { Granularity } from '../../model/series';

/** The categorical slots in fixed order, then the reserved gray for Other. */
export const SERIES_COLORS = [
  'var(--chart-1)',
  'var(--chart-2)',
  'var(--chart-3)',
  'var(--chart-4)',
  'var(--chart-5)',
] as const;
export const OTHER_COLOR = 'var(--sev-info)';

export interface Series {
  key: string;
  label: string;
  color: string;
}

/** Bucket ticks: a day, the Monday of a week, or a month. */
export function useBucketLabel(granularity: Granularity) {
  const format = useFormatter();
  // A bucket is a UTC calendar date; noon keeps it on that date in any zone.
  return (bucket: string) =>
    format.dateTime(
      new Date(`${bucket}T12:00:00Z`),
      granularity === 'month' ? 'axisMonth' : 'axisDay',
    );
}

/** A card holding one chart: its title, controls on the right, the plot. */
export function ChartCard({
  title,
  actions,
  children,
  className,
}: {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      className={cn(
        'flex flex-col gap-4 rounded-xl border bg-card p-5',
        className,
      )}
    >
      <header className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-sm font-medium">{title}</h2>
        {actions && (
          <div className="flex flex-wrap items-center gap-2">{actions}</div>
        )}
      </header>
      {children}
    </section>
  );
}

/** A headline number, its change against the previous period, and its trend. */
export function KpiTile({
  label,
  value,
  change,
  spark,
}: {
  label: string;
  value: string;
  /** Against the previous period, as a fraction; `null` when there was none. */
  change: number | null;
  spark: number[];
}) {
  const t = useTranslations('usage');
  const format = useFormatter();
  const Arrow = change !== null && change < 0 ? ArrowDownRight : ArrowUpRight;
  return (
    <div className="flex min-w-0 flex-col gap-1 rounded-xl border bg-card px-4 pt-3.5 pb-3">
      <p className="truncate text-xs font-medium text-muted-foreground">
        {label}
      </p>
      <p className="text-2xl font-semibold tracking-tight tabular-nums">
        {value}
      </p>
      <div className="flex items-center justify-between gap-2">
        <p className="flex min-w-0 items-center gap-1 truncate text-xs text-muted-foreground tabular-nums">
          {change === null ? (
            t('kpi.noPrevious')
          ) : (
            <>
              <Arrow className="size-3.5 shrink-0" />
              {t('kpi.vsPrevious', {
                change: format.number(change, 'percentChange'),
              })}
            </>
          )}
        </p>
        <Sparkline values={spark} />
      </div>
    </div>
  );
}

function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return null;
  const data = values.map((value, i) => ({ i, value }));
  return (
    <div className="h-7 w-16 shrink-0" aria-hidden>
      <ResponsiveContainer>
        <AreaChart
          data={data}
          margin={{ top: 2, right: 0, bottom: 2, left: 0 }}
        >
          <Area
            dataKey="value"
            type="monotone"
            stroke="var(--chart-1)"
            strokeWidth={1.5}
            fill="var(--chart-1)"
            fillOpacity={0.12}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

/** A legend that names every series next to its colour. */
export function Legend({ series }: { series: Series[] }) {
  return (
    <ul className="flex flex-wrap gap-x-4 gap-y-1.5 text-xs text-muted-foreground">
      {series.map((s) => (
        <li key={s.key} className="flex min-w-0 items-center gap-1.5">
          <span
            className="size-2.5 shrink-0 rounded-sm"
            style={{ background: s.color }}
          />
          <span className="max-w-48 truncate">{s.label}</span>
        </li>
      ))}
    </ul>
  );
}

const axisTick = { fill: 'var(--muted-foreground)', fontSize: 11 };

/** Every series of the hovered bucket, largest first, with the total. */
function StackTooltip({
  active,
  payload,
  label,
  series,
  format,
  bucketLabel,
  totalLabel,
}: {
  active?: boolean;
  payload?: { dataKey?: unknown; value?: unknown }[];
  label?: unknown;
  series: Series[];
  format: (value: number) => string;
  bucketLabel: (bucket: string) => string;
  totalLabel: string;
}) {
  if (!active || !payload?.length) return null;
  const rows = payload
    .map((p) => ({
      series: series.find((s) => s.key === p.dataKey),
      value: Number(p.value ?? 0),
    }))
    .filter((r) => r.series && r.value > 0)
    .sort((a, b) => b.value - a.value);
  const total = rows.reduce((sum, r) => sum + r.value, 0);
  return (
    <div className="min-w-44 rounded-lg border bg-popover px-3 py-2 text-xs shadow-md">
      <p className="mb-1.5 font-medium">{bucketLabel(String(label))}</p>
      <ul className="space-y-1">
        {rows.map((r) => (
          <li key={r.series?.key} className="flex items-center gap-2">
            <span
              className="size-2 shrink-0 rounded-sm"
              style={{ background: r.series?.color }}
            />
            <span className="max-w-40 truncate text-muted-foreground">
              {r.series?.label}
            </span>
            <span className="ml-auto pl-3 tabular-nums">{format(r.value)}</span>
          </li>
        ))}
      </ul>
      {rows.length > 1 && (
        <p className="mt-1.5 flex border-t pt-1.5 font-medium">
          {totalLabel}
          <span className="ml-auto tabular-nums">{format(total)}</span>
        </p>
      )}
    </div>
  );
}

/** Stacked bars over time, one segment per series, with a legend. */
export function StackedBars({
  rows,
  series,
  format,
  bucketLabel,
  height = 'h-72',
}: {
  rows: Record<string, number | string>[];
  series: Series[];
  format: (value: number) => string;
  bucketLabel: (bucket: string) => string;
  height?: string;
}) {
  const t = useTranslations('usage');
  return (
    <div className="space-y-3">
      <div className={cn('w-full', height)}>
        <ResponsiveContainer>
          <BarChart
            data={rows}
            margin={{ top: 4, right: 4, bottom: 0, left: 0 }}
          >
            <CartesianGrid vertical={false} stroke="var(--border)" />
            <XAxis
              dataKey="bucket"
              tickFormatter={bucketLabel}
              tick={axisTick}
              axisLine={false}
              tickLine={false}
              minTickGap={16}
            />
            <YAxis
              tickFormatter={format}
              tick={axisTick}
              axisLine={false}
              tickLine={false}
              width={72}
            />
            <Tooltip
              cursor={{ fill: 'var(--muted)', opacity: 0.5 }}
              content={
                <StackTooltip
                  series={series}
                  format={format}
                  bucketLabel={bucketLabel}
                  totalLabel={t('chart.total')}
                />
              }
            />
            {series.map((s, i) => (
              <Bar
                key={s.key}
                dataKey={s.key}
                stackId="stack"
                fill={s.color}
                // A 2px surface gap between segments; rounded only on top.
                stroke="var(--card)"
                strokeWidth={1}
                radius={i === series.length - 1 ? [4, 4, 0, 0] : 0}
                maxBarSize={48}
                isAnimationActive={false}
              />
            ))}
          </BarChart>
        </ResponsiveContainer>
      </div>
      {series.length > 1 && <Legend series={series} />}
    </div>
  );
}

/** One measure over time as a line; buckets without a value leave a gap. */
export function TrendLine({
  rows,
  format,
  bucketLabel,
  name,
}: {
  rows: { bucket: string; value: number | null }[];
  format: (value: number) => string;
  bucketLabel: (bucket: string) => string;
  name: string;
}) {
  return (
    <div className="h-56 w-full">
      <ResponsiveContainer>
        <LineChart
          data={rows}
          margin={{ top: 8, right: 8, bottom: 0, left: 0 }}
        >
          <CartesianGrid vertical={false} stroke="var(--border)" />
          <XAxis
            dataKey="bucket"
            tickFormatter={bucketLabel}
            tick={axisTick}
            axisLine={false}
            tickLine={false}
            minTickGap={16}
          />
          <YAxis
            tickFormatter={format}
            tick={axisTick}
            axisLine={false}
            tickLine={false}
            width={72}
          />
          <Tooltip
            cursor={{ stroke: 'var(--border)' }}
            contentStyle={{
              background: 'var(--popover)',
              border: '1px solid var(--border)',
              borderRadius: 8,
              fontSize: 12,
            }}
            labelStyle={{ color: 'var(--foreground)' }}
            labelFormatter={(bucket) => bucketLabel(String(bucket))}
            formatter={(value) => [format(Number(value)), name]}
          />
          <Line
            dataKey="value"
            type="monotone"
            stroke="var(--chart-1)"
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4, stroke: 'var(--card)', strokeWidth: 2 }}
            connectNulls={false}
            isAnimationActive={false}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
