import type { Totals, Usage } from '@/shared/api/schemas';

export type Granularity = 'day' | 'week' | 'month';
export const GRANULARITIES: readonly Granularity[] = ['day', 'week', 'month'];

export type Metric = 'spend' | 'requests' | 'tokens';
export const METRICS: readonly Metric[] = ['spend', 'requests', 'tokens'];

export type GroupBy = 'model' | 'team' | 'key' | 'person' | 'end_user' | 'tag';
export const GROUPS: readonly GroupBy[] = [
  'model',
  'team',
  'key',
  'person',
  'end_user',
  'tag',
];

/** How many groups get their own colour; the rest fold into "Other". */
export const TOP = 5;
export const OTHER = '__other__';

export function metricOf(totals: Totals, metric: Metric): number {
  switch (metric) {
    case 'spend':
      return totals.cost_usd;
    case 'requests':
      return totals.requests;
    case 'tokens':
      return totals.prompt_tokens + totals.completion_tokens;
  }
}

const DAY_MS = 24 * 60 * 60 * 1000;

/** Days for short ranges, weeks up to four months, months beyond. */
export function defaultGranularity(from: string, to: string): Granularity {
  const days = (Date.parse(to) - Date.parse(from)) / DAY_MS + 1;
  return days <= 31 ? 'day' : days <= 120 ? 'week' : 'month';
}

/** The bucket a `yyyy-mm-dd` day falls in: itself, its Monday, its month. */
export function bucketOf(day: string, granularity: Granularity): string {
  if (granularity === 'day') return day;
  if (granularity === 'month') return `${day.slice(0, 7)}-01`;
  const date = new Date(`${day}T00:00:00Z`);
  const back = (date.getUTCDay() + 6) % 7;
  return new Date(date.getTime() - back * DAY_MS).toISOString().slice(0, 10);
}

/** Every bucket of the range in order, empty ones included. */
export function bucketsOf(
  daily: readonly { day: string }[],
  granularity: Granularity,
): string[] {
  return [...new Set(daily.map((d) => bucketOf(d.day, granularity)))];
}

export const emptyTotals = (): Totals => ({
  cost_usd: 0,
  requests: 0,
  failed_requests: 0,
  prompt_tokens: 0,
  completion_tokens: 0,
  cached_tokens: 0,
  cache_write_tokens: 0,
  reasoning_tokens: 0,
  latency_ms: 0,
});

export function addTotals(into: Totals, add: Totals): Totals {
  for (const key of Object.keys(into) as (keyof Totals)[]) {
    into[key] += add[key];
  }
  return into;
}

/** The daily totals summed per bucket. */
export function bucketTotals(
  daily: Usage['daily'],
  granularity: Granularity,
): { bucket: string; totals: Totals }[] {
  const out = new Map<string, Totals>();
  for (const bucket of bucketsOf(daily, granularity)) {
    out.set(bucket, emptyTotals());
  }
  for (const day of daily) {
    addTotals(out.get(bucketOf(day.day, granularity)) as Totals, day);
  }
  return [...out].map(([bucket, totals]) => ({ bucket, totals }));
}

export interface Group {
  key: string;
  label: string | null;
  totals: Totals;
}

/** Each group's totals over the whole range, largest `metric` first. */
export function groupTotals(series: Usage['series'], metric: Metric): Group[] {
  const groups = new Map<string, Group>();
  for (const point of series) {
    const group = groups.get(point.key) ?? {
      key: point.key,
      label: point.label,
      totals: emptyTotals(),
    };
    addTotals(group.totals, point);
    groups.set(point.key, group);
  }
  return [...groups.values()].sort(
    (a, b) =>
      metricOf(b.totals, metric) - metricOf(a.totals, metric) ||
      a.key.localeCompare(b.key),
  );
}

/**
 * The stacked chart's rows: one per bucket, a value per top group and one
 * for the rest under `OTHER`. `keys` lists the top groups in name order, so
 * a group keeps its colour while others come and go.
 */
export function stack(
  usage: Pick<Usage, 'daily' | 'series'>,
  metric: Metric,
  granularity: Granularity,
): {
  keys: Group[];
  hasOther: boolean;
  rows: Record<string, number | string>[];
} {
  const ranked = groupTotals(usage.series, metric);
  const keys = ranked.slice(0, TOP).sort((a, b) => a.key.localeCompare(b.key));
  const top = new Set(keys.map((k) => k.key));
  const rows = new Map<string, Record<string, number | string>>();
  for (const bucket of bucketsOf(usage.daily, granularity)) {
    rows.set(bucket, { bucket });
  }
  for (const point of usage.series) {
    const row = rows.get(bucketOf(point.day, granularity));
    if (!row) continue;
    const column = top.has(point.key) ? point.key : OTHER;
    row[column] = Number(row[column] ?? 0) + metricOf(point, metric);
  }
  return { keys, hasOther: ranked.length > TOP, rows: [...rows.values()] };
}

/** The change from `previous` as a fraction; `null` when there was none. */
export function delta(current: number, previous: number): number | null {
  if (previous === 0) return null;
  return (current - previous) / previous;
}

/** Average latency in ms, or `null` with no requests. */
export function avgLatency(totals: Totals): number | null {
  return totals.requests > 0 ? totals.latency_ms / totals.requests : null;
}

export function errorRate(totals: Totals): number {
  return totals.requests > 0 ? totals.failed_requests / totals.requests : 0;
}

/**
 * Tokens split so the parts add up: prompt tokens include cache reads and
 * writes, completion tokens include reasoning.
 */
export function tokenParts(totals: Totals) {
  return {
    input: Math.max(0, totals.prompt_tokens - totals.cached_tokens),
    cached: totals.cached_tokens,
    output: Math.max(0, totals.completion_tokens - totals.reasoning_tokens),
    reasoning: totals.reasoning_tokens,
  };
}

/**
 * The groupings that still split something under the page's filters. One
 * team (or key, or person, each in one team) leaves nothing to split by who
 * is billed; people only mean something inside one team; customers and tags
 * are rolled up without the model or the person.
 */
export function availableGroups(filters: {
  team?: number;
  key?: number;
  model?: string;
  person?: number;
}): GroupBy[] {
  const { team, key, model, person } = filters;
  return GROUPS.filter((g) => {
    if (g === 'team') return !team && !key && !person;
    if (g === 'key') return !key;
    if (g === 'person') return Boolean(team) && !key && !person;
    if (g === 'model') return !model;
    return !model && !person;
  });
}

/** The grouping asked for when it still applies, else the first that does. */
export function effectiveGroup(
  wanted: GroupBy | undefined,
  filters: Parameters<typeof availableGroups>[0],
): GroupBy {
  const groups = availableGroups(filters);
  return wanted && groups.includes(wanted) ? wanted : groups[0];
}

export type Narrowing =
  | { filter: { model?: string; team?: number; key?: number; person?: number } }
  | { logs: { end_user: string } | { tag: string } }
  | null;

/**
 * What clicking a group does: narrow the page to it, or open the logs for a
 * customer or tag (which the page cannot filter by). A user's personal keys
 * and keys of no one have nothing to narrow to.
 */
export function narrowTo(group: GroupBy, key: string): Narrowing {
  const id = /^\d+$/.test(key) ? Number(key) : undefined;
  switch (group) {
    case 'model':
      return { filter: { model: key } };
    case 'end_user':
      return { logs: { end_user: key } };
    case 'tag':
      return { logs: { tag: key } };
    default:
      return id ? { filter: { [group]: id } } : null;
  }
}
