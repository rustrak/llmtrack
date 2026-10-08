import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { ChevronDown, Download, FileSpreadsheet, FileText } from 'lucide-react';
import { useState } from 'react';
import { useFormatter, useTranslations } from 'use-intl';
import { listKeys } from '@/features/key/api/queries';
import { listModels } from '@/features/model/api/queries';
import { listPeople, listTeams } from '@/features/team/api/queries';
import { getUsage } from '@/features/usage/api/queries';
import { usageCsv } from '@/features/usage/model/csv';
import {
  RANGE_PRESETS,
  type RangePreset,
  resolveRange,
} from '@/features/usage/model/range';
import {
  availableGroups,
  avgLatency,
  bucketTotals,
  defaultGranularity,
  delta,
  effectiveGroup,
  errorRate,
  GROUPS,
  type Granularity,
  type Group,
  type GroupBy,
  groupTotals,
  METRICS,
  type Metric,
  metricOf,
  narrowTo,
  OTHER,
  stack,
  tokenParts,
} from '@/features/usage/model/series';
import { ReportPanel } from '@/features/usage/ui/components/report-panel';
import {
  ChartCard,
  KpiTile,
  OTHER_COLOR,
  SERIES_COLORS,
  StackedBars,
  TrendLine,
  useBucketLabel,
} from '@/features/usage/ui/components/usage-charts';
import { UsageRanking } from '@/features/usage/ui/components/usage-ranking';
import { useLatency } from '@/features/usage/ui/components/use-latency';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { MAX_PAGE_SIZE } from '@/shared/lib/list-params';
import { Choice } from '@/shared/ui/components/choice';
import { FilterPill } from '@/shared/ui/components/data-table';
import { Page } from '@/shared/ui/components/page';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/components/shadcn/dropdown-menu';
import { Input } from '@/shared/ui/components/shadcn/input';
import {
  Tabs,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import { useMoney } from '@/shared/ui/hooks/use-money';

interface UsageSearch {
  range?: RangePreset;
  /** A custom range, both `yyyy-mm-dd`; wins over `range`. */
  from?: string;
  to?: string;
  team?: number;
  key?: number;
  model?: string;
  /** Someone of `team`: only kept with a team. */
  person?: number;
  group?: GroupBy;
  metric?: Metric;
}

const CUSTOM = 'custom';

export const Route = createFileRoute('/_authenticated/usage')({
  validateSearch: (search: Record<string, unknown>): UsageSearch => {
    const id = (value: unknown) => {
      const n = Number(value);
      return Number.isInteger(n) && n > 0 ? n : undefined;
    };
    const text = (value: unknown) =>
      typeof value === 'string' && value ? value : undefined;
    const from = text(search.from);
    const to = text(search.to);
    return {
      range: RANGE_PRESETS.find((p) => p === search.range),
      ...(from && to ? { from, to } : {}),
      team: id(search.team),
      key: id(search.key),
      model: text(search.model),
      person: id(search.team) ? id(search.person) : undefined,
      group: GROUPS.find((g) => g === search.group),
      metric: METRICS.find((m) => m === search.metric),
    };
  },
  loaderDeps: ({ search }) => ({
    range: search.range,
    from: search.from,
    to: search.to,
    team: search.team,
    key: search.key,
    model: search.model,
    person: search.person,
    group: effectiveGroup(search.group, search),
  }),
  loader: async ({ deps }) => {
    const [usage, teams, keys, models, people] = await Promise.all([
      getUsage({
        ...resolveRange(deps),
        team_id: deps.team,
        key_id: deps.key,
        model: deps.model,
        person_id: deps.person,
        group_by: deps.group,
      }),
      listTeams({ per_page: MAX_PAGE_SIZE }),
      // ponytail: the first 200 keys and models as filter options.
      listKeys({ per_page: MAX_PAGE_SIZE }),
      listModels({ per_page: MAX_PAGE_SIZE }),
      // People only mean something inside one team.
      deps.team
        ? listPeople({ team_id: deps.team, per_page: MAX_PAGE_SIZE })
        : null,
    ]);
    return {
      usage: unwrap(usage),
      teams: unwrap(teams).data,
      keys: unwrap(keys).data,
      models: [...new Set(unwrap(models).data.map((m) => m.name))],
      people: people ? unwrap(people).data : [],
    };
  },
  head: () => ({ meta: [{ title: translator('usage')('meta.title') }] }),
  component: UsagePage,
});

type Usage = ReturnType<typeof Route.useLoaderData>['usage'];
type Set = (patch: Partial<UsageSearch>) => void;

function UsagePage() {
  const t = useTranslations('usage');
  const { usage, teams, keys, people } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  // Undefined once the filters leave nothing to split by.
  const group = effectiveGroup(search.group, search) as GroupBy | undefined;
  const metric = search.metric ?? 'spend';
  // Bars per day up to a month, per week up to four, per month beyond.
  const granularity = defaultGranularity(usage.from, usage.to);
  const bucketLabel = useBucketLabel(granularity);
  const set: Set = (patch) =>
    navigate({ search: (prev) => ({ ...prev, ...patch }) });
  const groupNames = useGroupNames();
  const nameOf = (g: Pick<Group, 'key' | 'label'>) => {
    if (g.label) return g.label;
    if (group === 'team' && g.key.startsWith('user:'))
      return t('personalOf', { name: g.label ?? g.key.slice(5) });
    if (group === 'team') return t('deletedTeam', { id: g.key });
    if (group === 'key') return t('deletedKey', { id: g.key });
    if (group === 'person')
      return g.key ? t('deletedPerson', { id: g.key }) : t('noPerson');
    return g.key;
  };
  const pick = (g: Group) => {
    const next = group && narrowTo(group, g.key);
    if (!next) return;
    if ('filter' in next) set(next.filter);
    else navigate({ to: '/logs', search: { window: 'all', ...next.logs } });
  };
  const [reporting, setReporting] = useState(false);
  const { date } = useMoney();
  const teamName = teams.find((x) => x.id === search.team)?.name;
  const scopeLabels = [
    `${date(usage.from)} – ${date(usage.to)}`,
    ...(teamName ? [`${t('team')}: ${teamName}`] : []),
    ...(search.key
      ? [
          `${t('key')}: ${keys.find((k) => k.id === search.key)?.name ?? `#${search.key}`}`,
        ]
      : []),
    ...(search.model ? [`${t('model')}: ${search.model}`] : []),
    ...(search.person
      ? [
          `${t('group.person')}: ${people.find((p) => p.id === search.person)?.name ?? `#${search.person}`}`,
        ]
      : []),
  ];
  if (scopeLabels.length === 1) scopeLabels.push(t('report.allUsage'));
  const download = () => {
    const blob = new Blob([usageCsv(usage, t('personal'))], {
      type: 'text/csv',
    });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = `llmtrack-usage-${usage.from}-${usage.to}.csv`;
    link.click();
    URL.revokeObjectURL(link.href);
  };

  return (
    <Page
      title={t('title')}
      description={t('subtitle')}
      actions={
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="outline" />}>
            <Download />
            {t('export')}
            <ChevronDown />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => setReporting(true)}>
              <FileText />
              {t('exportReport')}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={download}>
              <FileSpreadsheet />
              {t('exportCsv')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      }
    >
      <ReportPanel
        // Fresh choices whenever the filters change under it.
        key={`${usage.from}:${usage.to}:${search.team}:${search.key}:${search.model}:${search.person}`}
        open={reporting}
        onOpenChange={setReporting}
        teamName={teamName}
        scopeLabel={scopeLabels.join(' · ')}
        scope={{
          from: usage.from,
          to: usage.to,
          team_id: search.team,
          key_id: search.key,
          model: search.model,
          person_id: search.person,
        }}
      />
      <UsageToolbar bucketLabel={bucketLabel} set={set} />
      <KpiRow usage={usage} />
      <MainChart
        usage={usage}
        group={group}
        metric={metric}
        granularity={granularity}
        bucketLabel={bucketLabel}
        set={set}
        nameOf={nameOf}
      />
      <DetailCharts
        usage={usage}
        granularity={granularity}
        bucketLabel={bucketLabel}
      />
      {group && (
        <section className="space-y-3">
          <h2 className="text-sm font-medium">
            {t('ranking', { group: groupNames[group] })}
          </h2>
          <UsageRanking
            groups={groupTotals(usage.series, metric)}
            metric={metric}
            groupLabel={groupNames[group]}
            nameOf={nameOf}
            onPick={pick}
            empty={t('table.empty')}
          />
        </section>
      )}
    </Page>
  );
}

function useGroupNames(): Record<GroupBy, string> {
  const t = useTranslations('usage');
  return {
    model: t('group.model'),
    team: t('group.team'),
    key: t('group.key'),
    person: t('group.person'),
    end_user: t('group.end_user'),
    tag: t('group.tag'),
  };
}

/** Range, interval and filters, in one row above everything they change. */
function UsageToolbar({
  bucketLabel,
  set,
}: {
  bucketLabel: (bucket: string) => string;
  set: Set;
}) {
  const t = useTranslations('usage');
  const tTable = useTranslations('table');
  const { usage, teams, keys, models } = Route.useLoaderData();
  const search = Route.useSearch();
  const { people } = Route.useLoaderData();
  const filtered = Boolean(
    search.team || search.key || search.model || search.person,
  );
  const custom = search.from !== undefined;
  const rangeLabels: Record<RangePreset, string> = {
    '7d': t('range.days7'),
    '30d': t('range.days30'),
    '90d': t('range.days90'),
    month: t('range.month'),
    'last-month': t('range.lastMonth'),
  };
  return (
    <div className="flex flex-wrap items-center gap-2">
      <Choice
        aria-label={t('dateRange')}
        className="h-8 w-44"
        value={custom ? CUSTOM : (search.range ?? '30d')}
        onChange={(value) =>
          value === CUSTOM
            ? set({ from: usage.from, to: usage.to })
            : set({
                range: value as RangePreset,
                from: undefined,
                to: undefined,
              })
        }
        options={[
          ...RANGE_PRESETS.map((value) => ({
            value,
            label: rangeLabels[value],
          })),
          { value: CUSTOM, label: t('range.custom') },
        ]}
      />
      {custom && (
        <>
          <Input
            type="date"
            aria-label={t('range.from')}
            className="h-8 w-40"
            value={usage.from}
            max={usage.to}
            onChange={(e) => e.target.value && set({ from: e.target.value })}
          />
          <Input
            type="date"
            aria-label={t('range.to')}
            className="h-8 w-40"
            value={usage.to}
            min={usage.from}
            onChange={(e) => e.target.value && set({ to: e.target.value })}
          />
        </>
      )}
      <span className="mx-1 hidden h-5 w-px bg-border sm:block" />
      <FilterPill
        label={t('team')}
        value={search.team ? String(search.team) : undefined}
        // Another team's people and keys do not carry over.
        onChange={(v) =>
          set({
            team: v ? Number(v) : undefined,
            person: undefined,
            key: undefined,
          })
        }
        options={teams.map((x) => ({ value: String(x.id), label: x.name }))}
      />
      {search.team && (
        <FilterPill
          label={t('person')}
          value={search.person ? String(search.person) : undefined}
          onChange={(v) => set({ person: v ? Number(v) : undefined })}
          options={people.map((p) => ({ value: String(p.id), label: p.name }))}
        />
      )}
      <FilterPill
        label={t('key')}
        value={search.key ? String(search.key) : undefined}
        onChange={(v) => set({ key: v ? Number(v) : undefined })}
        options={keys
          .filter((k) => !search.team || k.team_id === search.team)
          .map((k) => ({ value: String(k.id), label: k.name }))}
      />
      <FilterPill
        label={t('model')}
        value={search.model}
        onChange={(model) => set({ model })}
        options={models.map((name) => ({ value: name, label: name }))}
      />
      {filtered && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() =>
            set({
              team: undefined,
              key: undefined,
              model: undefined,
              person: undefined,
            })
          }
        >
          {tTable('clearFilters')}
        </Button>
      )}
      <span className="ml-auto text-xs text-muted-foreground">
        {t('kpi.comparedWith', {
          from: bucketLabel(previousFrom(usage.from, usage.to)),
          to: bucketLabel(previousTo(usage.from)),
        })}
      </span>
    </div>
  );
}

/** The headline numbers, each against the previous period, with its trend. */
function KpiRow({ usage }: { usage: Usage }) {
  const t = useTranslations('usage');
  const money = useMoney();
  const format = useFormatter();
  const latency = useLatency();
  const { totals, previous } = usage;
  type T = typeof totals;
  const tokens = (x: T) => x.prompt_tokens + x.completion_tokens;
  const costPer1k = (x: T) =>
    x.requests > 0 ? (x.cost_usd / x.requests) * 1000 : 0;
  const latencyOf = (x: T) => avgLatency(x) ?? 0;
  const days = bucketTotals(usage.daily, 'day');
  const spark = (pick: (x: T) => number) => days.map((d) => pick(d.totals));
  const tile = (
    label: string,
    pick: (x: T) => number,
    show: (value: number) => string,
  ) => (
    <KpiTile
      label={label}
      value={show(pick(totals))}
      change={totals.requests > 0 ? delta(pick(totals), pick(previous)) : null}
      spark={spark(pick)}
    />
  );
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-6">
      {tile(t('spend'), (x) => x.cost_usd, money.usd)}
      {tile(t('requests'), (x) => x.requests, money.compact)}
      {tile(t('errorRate'), errorRate, (v) => format.number(v, 'percent'))}
      {tile(t('tokens'), tokens, money.compact)}
      {tile(t('avgLatency'), latencyOf, (v) =>
        totals.requests > 0 ? latency(v) : '—',
      )}
      {tile(t('costPer1k'), costPer1k, money.usd)}
    </div>
  );
}

/**
 * The chosen measure over time, split by the chosen grouping: the filters
 * decide what is counted, the split only how it is coloured.
 */
function MainChart({
  usage,
  group,
  metric,
  granularity,
  bucketLabel,
  set,
  nameOf,
}: {
  usage: Usage;
  group: GroupBy | undefined;
  metric: Metric;
  granularity: Granularity;
  bucketLabel: (bucket: string) => string;
  set: Set;
  nameOf: (g: Pick<Group, 'key' | 'label'>) => string;
}) {
  const t = useTranslations('usage');
  const money = useMoney();
  const groupNames = useGroupNames();
  const search = Route.useSearch();
  const groups = availableGroups(search);
  const format = (v: number) =>
    metric === 'spend' ? money.usd(v) : money.compact(v);
  const stacked = stack(usage, metric, granularity);
  const series = group
    ? [
        ...stacked.keys.map((k, i) => ({
          key: k.key,
          label: nameOf(k),
          color: SERIES_COLORS[i],
        })),
        ...(stacked.hasOther
          ? [{ key: OTHER, label: t('chart.other'), color: OTHER_COLOR }]
          : []),
      ]
    : [{ key: 'value', label: t(`metric.${metric}`), color: SERIES_COLORS[0] }];
  const rows = group
    ? stacked.rows
    : bucketTotals(usage.daily, granularity).map((b) => ({
        bucket: b.bucket,
        value: metricOf(b.totals, metric),
      }));
  const empty = usage.totals.requests === 0;

  return (
    <ChartCard
      title={
        group
          ? t('chartTitle', {
              metric: t(`metric.${metric}`),
              group: groupNames[group],
            })
          : t(`metric.${metric}`)
      }
      actions={
        <Tabs
          value={metric}
          onValueChange={(value) =>
            set({ metric: METRICS.find((m) => m === value) })
          }
        >
          <TabsList>
            {METRICS.map((value) => (
              <TabsTrigger key={value} value={value}>
                {t(`metric.${value}`)}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      }
    >
      {group && groups.length > 1 && (
        <div className="-mt-1 flex flex-wrap items-center gap-1 text-sm">
          <span className="mr-1 text-muted-foreground">{t('splitBy')}</span>
          {groups.map((value) => (
            <Button
              key={value}
              size="sm"
              variant={value === group ? 'secondary' : 'ghost'}
              aria-pressed={value === group}
              className="h-7 capitalize"
              onClick={() => set({ group: value })}
            >
              {groupNames[value]}
            </Button>
          ))}
        </div>
      )}
      {empty || series.length === 0 ? (
        <p className="flex h-72 items-center justify-center text-sm text-muted-foreground">
          {t('table.empty')}
        </p>
      ) : (
        <StackedBars
          rows={rows}
          series={series}
          format={format}
          bucketLabel={bucketLabel}
        />
      )}
    </ChartCard>
  );
}

/** Requests and errors, tokens by type, latency: each on its own axis. */
function DetailCharts({
  usage,
  granularity,
  bucketLabel,
}: {
  usage: Usage;
  granularity: Granularity;
  bucketLabel: (bucket: string) => string;
}) {
  const t = useTranslations('usage');
  const money = useMoney();
  const latency = useLatency();
  const buckets = bucketTotals(usage.daily, granularity);
  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <ChartCard title={t('chart.requests')}>
        <StackedBars
          height="h-56"
          rows={buckets.map((b) => ({
            bucket: b.bucket,
            ok: b.totals.requests - b.totals.failed_requests,
            failed: b.totals.failed_requests,
          }))}
          series={[
            { key: 'ok', label: t('chart.ok'), color: 'var(--chart-1)' },
            {
              key: 'failed',
              label: t('chart.failed'),
              color: 'var(--sev-error)',
            },
          ]}
          format={(v) => money.compact(v)}
          bucketLabel={bucketLabel}
        />
      </ChartCard>
      <ChartCard title={t('chart.tokens')}>
        <StackedBars
          height="h-56"
          rows={buckets.map((b) => ({
            bucket: b.bucket,
            ...tokenParts(b.totals),
          }))}
          series={(['input', 'cached', 'output', 'reasoning'] as const).map(
            (key, i) => ({
              key,
              label: t(`chart.${key}`),
              color: SERIES_COLORS[i],
            }),
          )}
          format={(v) => money.compact(v)}
          bucketLabel={bucketLabel}
        />
      </ChartCard>
      <ChartCard title={t('chart.latency')}>
        <TrendLine
          rows={buckets.map((b) => ({
            bucket: b.bucket,
            value: avgLatency(b.totals),
          }))}
          format={latency}
          bucketLabel={bucketLabel}
          name={t('avgLatency')}
        />
      </ChartCard>
    </div>
  );
}

const DAY_MS = 24 * 60 * 60 * 1000;
const iso = (ms: number) => new Date(ms).toISOString().slice(0, 10);
/** The first day of the equal window before `from`–`to`, as the server takes it. */
const previousFrom = (from: string, to: string) =>
  iso(2 * Date.parse(from) - Date.parse(to) - DAY_MS);
const previousTo = (from: string) => iso(Date.parse(from) - DAY_MS);
