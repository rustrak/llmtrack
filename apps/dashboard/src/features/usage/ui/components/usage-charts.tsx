import {
  ArrowRightIcon,
  ChartLegend,
  cn,
  Sparkline,
  Text,
  TimeSeriesChart,
  TrendDownIcon,
  TrendUpIcon,
} from '@rustrak/ui';
import type { ReactNode } from 'react';
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { useFormatter, useTranslations } from 'use-intl';
import type { Granularity } from '../../model/series';

/** The categorical slots in fixed order, then the reserved gray for Other. */
export const SERIES_COLORS = [
  'var(--chart-1)',
  'var(--chart-2)',
  'var(--chart-3)',
  'var(--chart-4)',
  'var(--chart-5)',
] as const;
export const OTHER_COLOR = 'var(--chart-muted)';

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
        'flex min-w-0 flex-col gap-4 rounded-2xl border border-border-subtle bg-surface p-5',
        className,
      )}
    >
      <header className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-card-title text-fg">{title}</h2>
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
  const Arrow =
    change === null || change === 0
      ? ArrowRightIcon
      : change > 0
        ? TrendUpIcon
        : TrendDownIcon;
  return (
    <div className="flex min-w-0 flex-col justify-between gap-3 rounded-xl border border-border-subtle bg-panel p-4">
      <Text variant="column" tone="subtle" truncate>
        {label}
      </Text>
      <div className="flex flex-col gap-1.5">
        <Text
          variant="numeric-lg"
          render={<p />}
          className="truncate leading-none normal-nums"
        >
          {value}
        </Text>
        <div className="flex items-end justify-between gap-2">
          <span className="flex min-w-0 items-center gap-0.5 truncate text-fg-subtle text-meta tabular-nums">
            {change === null ? (
              t('kpi.noPrevious')
            ) : (
              <>
                <Arrow size="sm" aria-hidden className="shrink-0" />
                {t('kpi.vsPrevious', {
                  change: format.number(change, 'percentChange'),
                })}
              </>
            )}
          </span>
          {spark.length > 1 && (
            <Sparkline
              values={spark}
              label={label}
              width={64}
              height={24}
              minScale={Number.MIN_VALUE}
              className="shrink-0"
            />
          )}
        </div>
      </div>
    </div>
  );
}

/** A legend that names every series next to its colour (two or more). */
export function Legend({ series }: { series: Series[] }) {
  return <ChartLegend series={series} />;
}

/* Axis ticks as the design system draws them: mono, ghost ink. */
function Tick({
  x,
  y,
  payload,
  format,
  axis,
}: {
  x?: number;
  y?: number;
  payload?: { value: string | number };
  format: (value: string) => string;
  axis: 'x' | 'y';
}) {
  if (payload == null) return null;
  return (
    <text
      x={x}
      y={y}
      dy={axis === 'x' ? 12 : 3}
      textAnchor={axis === 'x' ? 'middle' : 'end'}
      className="fill-fg-ghost font-mono text-mono-sm tabular-nums"
    >
      {format(String(payload.value))}
    </text>
  );
}

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
    <div
      role="status"
      className="min-w-44 rounded-md border border-border bg-surface-floating px-2.5 py-2 shadow-overlay"
    >
      <p className="pb-1.5 font-mono text-fg-subtle text-mono-sm">
        {bucketLabel(String(label))}
      </p>
      {rows.map((r) => (
        <div key={r.series?.key} className="flex items-center gap-2 py-0.5">
          <span
            aria-hidden
            className="size-dot shrink-0 rounded-pill"
            style={{ background: r.series?.color }}
          />
          <span className="min-w-0 max-w-40 flex-1 truncate text-fg-muted text-meta">
            {r.series?.label}
          </span>
          <span className="shrink-0 pl-3 font-mono text-fg text-mono-sm tabular-nums">
            {format(r.value)}
          </span>
        </div>
      ))}
      {rows.length > 1 && (
        <div className="mt-1 flex items-center gap-2 border-border-divider border-t pt-1.5">
          <span className="flex-1 text-fg text-meta">{totalLabel}</span>
          <span className="font-mono text-fg text-mono-sm tabular-nums">
            {format(total)}
          </span>
        </div>
      )}
    </div>
  );
}

/**
 * Stacked bars over time, one segment per series, with a legend. Drawn here
 * rather than with the design system's `BarsChart` for the tooltip's total.
 */
export function StackedBars({
  rows,
  series,
  format,
  bucketLabel,
  height = 288,
}: {
  rows: Record<string, number | string>[];
  series: Series[];
  format: (value: number) => string;
  bucketLabel: (bucket: string) => string;
  /** In pixels. */
  height?: number;
}) {
  const t = useTranslations('usage');
  return (
    <figure className="min-w-0">
      <ResponsiveContainer width="100%" height={height}>
        <BarChart
          data={rows}
          margin={{ top: 4, right: 4, bottom: 0, left: 0 }}
          barCategoryGap="24%"
        >
          <CartesianGrid
            vertical={false}
            stroke="var(--border-divider)"
            strokeDasharray="3 3"
          />
          <XAxis
            dataKey="bucket"
            tick={<Tick axis="x" format={bucketLabel} />}
            axisLine={false}
            tickLine={false}
            interval="preserveStartEnd"
            minTickGap={32}
          />
          <YAxis
            tick={<Tick axis="y" format={(v) => format(Number(v))} />}
            axisLine={false}
            tickLine={false}
            tickCount={4}
            width={72}
          />
          <Tooltip
            cursor={{ fill: 'var(--surface-hover)' }}
            isAnimationActive={false}
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
              // The 1 px surface seam between bands; rounded only on top.
              stroke="var(--surface)"
              strokeWidth={1}
              radius={i === series.length - 1 ? [2, 2, 0, 0] : 0}
              maxBarSize={48}
              isAnimationActive={false}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
      <Legend series={series} />
    </figure>
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
    <TimeSeriesChart
      data={rows}
      series={[{ key: 'value', label: name }]}
      xKey="bucket"
      height={224}
      formatX={(bucket) => bucketLabel(String(bucket))}
      formatY={format}
      label={name}
    />
  );
}
