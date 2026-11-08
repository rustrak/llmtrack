import {
  Button,
  createDataTableColumnHelper,
  DataTable,
  ExportIcon,
  Menu,
  PayloadIcon,
  SegmentedControl,
  SegmentedItem,
} from '@rustrak/ui';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import { useTranslations } from 'use-intl';
import { listKeys } from '@/features/key/api/queries';
import { listLabels } from '@/features/label/api/queries';
import {
  type ExportFormat,
  exportUrl,
  getLogs,
  LOG_FILTERS,
  type LogFilter,
} from '@/features/log/api/queries';
import {
  LOG_WINDOWS,
  type LogWindow,
  parseWindow,
  since,
} from '@/features/log/model/window';
import {
  billedTo,
  LogDetail,
  StatusBadge,
} from '@/features/log/ui/components/log-detail';
import { TextFilter } from '@/features/log/ui/components/text-filter';
import { listModels } from '@/features/model/api/queries';
import { listTeams } from '@/features/team/api/queries';
import { unwrap } from '@/shared/api/http';
import type { LogLine } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import {
  filterParams,
  type ListFilter,
  readFilter,
} from '@/shared/lib/filters';
import { MAX_PAGE_SIZE, parseListParams } from '@/shared/lib/list-params';
import {
  FilterPill,
  ListFilterPill,
  ListQueryBar,
} from '@/shared/ui/components/list-filters';
import { Page } from '@/shared/ui/components/page';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';
import {
  KeysIcon,
  LabelsIcon,
  ModelsIcon,
  PlaygroundIcon,
  TeamsIcon,
} from '@/shared/ui/icons';

type LogSearch = Omit<LogFilter, 'from'> & { window: LogWindow };
/** What `GET /api/logs` sorts by when not told. */
const DEFAULT_SORT = '-created_at';

export const Route = createFileRoute('/_authenticated/logs')({
  validateSearch: (search: Record<string, unknown>): LogSearch => {
    const ids = new Set<string>(['team_id', 'key_id', 'label_id']);
    return {
      ...parseListParams(search),
      window: parseWindow(search.window),
      ...Object.assign(
        {},
        ...LOG_FILTERS.map((name) =>
          filterParams(name, readFilter(search, name, ids.has(name))),
        ),
      ),
      ...(search.status === 'success' || search.status === 'error'
        ? { status: search.status }
        : {}),
      ...(typeof search.text === 'string' && search.text.trim()
        ? { text: search.text.trim() }
        : {}),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: async ({ deps: { window, ...filter } }) => {
    const [page, teams, models, keys, labels] = await Promise.all([
      getLogs({ ...filter, from: since(window) }),
      listTeams({ per_page: MAX_PAGE_SIZE }),
      listModels({ per_page: MAX_PAGE_SIZE }),
      // ponytail: the first 200 keys as filter options; a search box past that.
      listKeys({ per_page: MAX_PAGE_SIZE }),
      listLabels({ per_page: MAX_PAGE_SIZE }),
    ]);
    return {
      page: unwrap(page),
      teams: unwrap(teams).data,
      models: [...new Set(unwrap(models).data.map((m) => m.name))],
      keys: unwrap(keys).data,
      labels: unwrap(labels).data,
    };
  },
  head: () => ({ meta: [{ title: translator('logs')('meta.title') }] }),
  component: LogsPage,
});

function Tokens({ row }: { row: LogLine }) {
  const t = useTranslations('logs');
  const money = useMoney();
  const extras = [
    row.cached_tokens > 0 &&
      t('cached', { count: money.compact(row.cached_tokens) }),
    row.cache_write_tokens > 0 &&
      t('cacheWritten', { count: money.compact(row.cache_write_tokens) }),
    row.reasoning_tokens > 0 &&
      t('reasoning', { count: money.compact(row.reasoning_tokens) }),
  ].filter(Boolean);
  return (
    <div className="text-right">
      <p className="font-mono text-mono-sm tabular-nums">
        {money.compact(row.prompt_tokens)} →{' '}
        {money.compact(row.completion_tokens)}
      </p>
      {extras.length > 0 && (
        <p className="text-fg-subtle text-meta">{extras.join(' · ')}</p>
      )}
    </div>
  );
}

const column = createDataTableColumnHelper<LogLine>();

/**
 * The request log as the design system's table: server-paged and
 * server-sorted, the column ids being the API's sort names. A row opens its
 * detail panel.
 */
function LogsTable({
  page,
  params,
  onParams,
  onOpen,
  empty,
}: {
  page: { data: LogLine[]; total: number };
  params: LogSearch;
  onParams: (patch: Partial<LogSearch>) => void;
  onOpen: (row: LogLine) => void;
  empty: { title: string; description?: string };
}) {
  const t = useTranslations('logs');
  const money = useMoney();

  const columns = useMemo(
    () =>
      column.columns([
        column.accessor('created_at', {
          id: 'created_at',
          header: t('table.time'),
          enableHiding: false,
          cell: ({ getValue }) => (
            <span className="whitespace-nowrap font-mono text-fg-subtle text-mono-sm">
              {money.dateTime(getValue())}
            </span>
          ),
          meta: { label: t('table.time'), width: 168 },
        }),
        column.accessor('status_code', {
          id: 'status',
          header: t('table.status'),
          enableSorting: false,
          cell: ({ getValue }) => <StatusBadge code={getValue()} />,
          meta: { label: t('table.status'), width: 96 },
        }),
        column.accessor('model_name', {
          id: 'model',
          header: t('table.model'),
          enableSorting: false,
          enableHiding: false,
          cell: ({ row }) => {
            const log = row.original;
            return (
              <div className="flex min-w-0 flex-col gap-0.5 py-1.5">
                <span className="truncate font-mono text-fg text-mono-sm">
                  {log.model_name}
                </span>
                <span className="truncate text-fg-subtle text-meta">
                  <span className="font-mono">/v1/{log.endpoint}</span>
                  {log.stream && ` · ${t('stream')}`}
                </span>
                {log.error && (
                  <span className="truncate text-danger-fg text-meta">
                    {log.error}
                  </span>
                )}
              </div>
            );
          },
          meta: { label: t('table.model') },
        }),
        column.display({
          id: 'team',
          header: t('table.team'),
          enableSorting: false,
          cell: ({ row }) => billedTo(row.original, t('personal')),
          meta: { label: t('table.team'), width: 180 },
        }),
        column.display({
          id: 'key',
          header: t('table.key'),
          enableSorting: false,
          cell: ({ row }) => {
            const log = row.original;
            return (
              <div className="flex min-w-0 flex-col gap-0.5 py-1.5">
                <span className="truncate">
                  {log.key_name ?? `#${log.key_id}`}
                </span>
                <span className="truncate font-mono text-fg-subtle text-mono-sm">
                  {log.key_hint}
                </span>
                {log.end_user && (
                  <span className="truncate text-fg-subtle text-meta">
                    {t('forEndUser', { user: log.end_user })}
                  </span>
                )}
              </div>
            );
          },
          meta: { label: t('table.key'), width: 180 },
        }),
        column.display({
          id: 'tokens',
          header: t('table.tokens'),
          cell: ({ row }) => <Tokens row={row.original} />,
          meta: {
            label: t('table.tokens'),
            width: 160,
            align: 'end',
            numeric: true,
          },
        }),
        column.accessor('cost_usd', {
          id: 'cost',
          header: t('table.cost'),
          cell: ({ getValue }) => (
            <span className="font-mono text-fg text-mono-sm tabular-nums">
              {money.usd(getValue())}
            </span>
          ),
          meta: {
            label: t('table.cost'),
            width: 112,
            align: 'end',
            numeric: true,
          },
        }),
        column.accessor('latency_ms', {
          id: 'latency',
          header: t('table.latency'),
          cell: ({ getValue }) => (
            <span className="font-mono text-fg-subtle text-mono-sm tabular-nums">
              {t('ms', { value: getValue() })}
            </span>
          ),
          meta: {
            label: t('table.latency'),
            width: 104,
            align: 'end',
            numeric: true,
          },
        }),
      ]),
    [t, money],
  );

  const table = useListTable({
    data: page.data,
    columns,
    total: page.total,
    params,
    defaultSort: DEFAULT_SORT,
    onParams,
    getRowId: (row) => String(row.id),
    storageKey: 'logs',
  });

  return (
    <DataTable
      table={table}
      className="min-h-0 flex-1"
      empty={empty}
      onRowClick={(row) => onOpen(row.original)}
    />
  );
}

function LogsPage() {
  const t = useTranslations('logs');
  const tTable = useTranslations('table');
  const { page, teams, models, keys, labels } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const [open, setOpen] = useState<LogLine | null>(null);
  const windowLabels: Record<LogWindow, string> = {
    '1h': t('window.hour'),
    '24h': t('window.day'),
    '7d': t('window.week'),
    '30d': t('window.month'),
    all: t('window.all'),
  };
  // Any change but the page itself starts again from the first page.
  const set = (patch: Partial<LogSearch>) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(
    search.q ||
      search.text ||
      search.status ||
      LOG_FILTERS.some(
        (name) =>
          search[name] !== undefined || search[`exclude_${name}`] !== undefined,
      ),
  );
  const filter = (name: (typeof LOG_FILTERS)[number]) => ({
    filter: readFilter(search, name),
    onChange: (next: ListFilter) => set(filterParams(name, next)),
  });
  const options = (rows: readonly { id: number; name: string }[]) =>
    rows.map((x) => ({ value: String(x.id), label: x.name }));
  const teamOptions = options(teams);
  const keyOptions = options(keys);
  const modelOptions = models.map((name) => ({ value: name, label: name }));
  const labelOptions = options(labels);
  const statusOptions = [
    { value: 'success', label: t('filters.success') },
    { value: 'error', label: t('filters.errors') },
  ];
  /** The stored request content behind what the filters show. */
  const download = (format: ExportFormat) => {
    const { window, ...filter } = search;
    const link = document.createElement('a');
    link.href = exportUrl({ ...filter, from: since(window) }, format);
    link.click();
  };

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        <Menu
          align="end"
          trigger={
            <Button icon={ExportIcon} menu>
              {t('export.button')}
            </Button>
          }
          actions={[
            {
              id: 'json',
              label: t('export.json'),
              icon: PayloadIcon,
              onSelect: () => download('json'),
            },
            {
              id: 'chat',
              label: t('export.chat'),
              icon: PlaygroundIcon,
              onSelect: () => download('chat'),
            },
          ]}
        />
      }
    >
      <div className="flex min-h-0 flex-1 flex-col gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <ListQueryBar
            className="order-first min-w-0 basis-full"
            placeholder={t('search')}
            search={search}
            onChange={(patch) => set(patch as Partial<LogSearch>)}
            fields={[
              {
                key: 'team_id',
                label: t('filters.team'),
                icon: TeamsIcon,
                variant: 'options',
                options: teamOptions,
                exclude: true,
              },
              {
                key: 'key_id',
                label: t('filters.key'),
                icon: KeysIcon,
                variant: 'options',
                options: keyOptions,
                exclude: true,
              },
              {
                key: 'model',
                label: t('filters.model'),
                icon: ModelsIcon,
                variant: 'options',
                options: modelOptions,
                exclude: true,
              },
              {
                key: 'label_id',
                label: t('filters.label'),
                icon: LabelsIcon,
                variant: 'options',
                options: labelOptions,
                exclude: true,
              },
              {
                key: 'status',
                label: t('filters.status'),
                variant: 'options',
                multiple: false,
                options: statusOptions.map((o) => ({
                  ...o,
                  tone: o.value === 'success' ? 'brand' : 'error',
                })),
              },
              {
                key: 'end_user',
                label: t('filters.endUser'),
                variant: 'text',
                exclude: true,
              },
              {
                key: 'tag',
                label: t('filters.tag'),
                variant: 'text',
                exclude: true,
              },
              {
                key: 'text',
                label: t('searchConversations'),
                variant: 'text',
              },
            ]}
          />
          <SegmentedControl
            aria-label={t('filters.window')}
            value={search.window}
            onValueChange={(v) => set({ window: parseWindow(v) })}
          >
            {LOG_WINDOWS.map((value) => (
              <SegmentedItem key={value} value={value}>
                {windowLabels[value]}
              </SegmentedItem>
            ))}
          </SegmentedControl>
          <ListFilterPill
            label={t('filters.team')}
            {...filter('team_id')}
            options={teamOptions}
          />
          <ListFilterPill
            label={t('filters.key')}
            {...filter('key_id')}
            options={keyOptions}
          />
          <ListFilterPill
            label={t('filters.model')}
            {...filter('model')}
            options={modelOptions}
          />
          <ListFilterPill
            label={t('filters.label')}
            {...filter('label_id')}
            options={labelOptions}
          />
          <FilterPill
            label={t('filters.status')}
            value={search.status}
            onChange={(v) =>
              set({
                status: v === 'success' || v === 'error' ? v : undefined,
              })
            }
            options={statusOptions}
          />
          <TextFilter label={t('filters.endUser')} {...filter('end_user')} />
          <TextFilter label={t('filters.tag')} {...filter('tag')} />
          {filtered && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() =>
                set({
                  q: undefined,
                  text: undefined,
                  status: undefined,
                  ...Object.assign(
                    {},
                    ...LOG_FILTERS.map((name) =>
                      filterParams(name, { values: [], exclude: false }),
                    ),
                  ),
                })
              }
            >
              {tTable('clearFilters')}
            </Button>
          )}
        </div>
        <LogsTable
          page={page}
          params={search}
          onParams={(patch) =>
            navigate({ search: (prev) => ({ ...prev, ...patch }) })
          }
          onOpen={setOpen}
          empty={
            filtered
              ? {
                  title: tTable('noResults'),
                  description: tTable('noResultsHint'),
                }
              : {
                  title: t('empty.title'),
                  description: t('empty.description'),
                }
          }
        />
      </div>
      <LogDetail row={open} onClose={() => setOpen(null)} />
    </Page>
  );
}
