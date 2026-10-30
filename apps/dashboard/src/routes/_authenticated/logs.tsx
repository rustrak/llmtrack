import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { ChevronDown, Download, FileJson, MessagesSquare } from 'lucide-react';
import { useState } from 'react';
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
import { Choice } from '@/shared/ui/components/choice';
import {
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
  FilterPill,
  ListFilterPill,
  ListFooter,
  SearchField,
  SortHead,
} from '@/shared/ui/components/data-table';
import { Page } from '@/shared/ui/components/page';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/components/shadcn/dropdown-menu';
import { useMoney } from '@/shared/ui/hooks/use-money';

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
      <p className="tabular-nums">
        {money.compact(row.prompt_tokens)} →{' '}
        {money.compact(row.completion_tokens)}
      </p>
      {extras.length > 0 && (
        <p className="text-xs text-muted-foreground">{extras.join(' · ')}</p>
      )}
    </div>
  );
}

function LogsPage() {
  const t = useTranslations('logs');
  const tTable = useTranslations('table');
  const { page, teams, models, keys, labels } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const money = useMoney();
  const [open, setOpen] = useState<LogLine | null>(null);
  const windowLabels: Record<LogWindow, string> = {
    '1h': t('window.hour'),
    '24h': t('window.day'),
    '7d': t('window.week'),
    '30d': t('window.month'),
    all: t('window.all'),
  };
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
  /** The stored request content behind what the filters show. */
  const download = (format: ExportFormat) => {
    const { window, ...filter } = search;
    const link = document.createElement('a');
    link.href = exportUrl({ ...filter, from: since(window) }, format);
    link.click();
  };
  const sortable = (field: string, label: string, className?: string) => (
    <SortHead
      field={field}
      sort={search.sort}
      fallback={DEFAULT_SORT}
      onSort={(sort) => set({ sort })}
      className={className}
    >
      {label}
    </SortHead>
  );

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        <DropdownMenu>
          <DropdownMenuTrigger render={<Button variant="outline" />}>
            <Download />
            {t('export.button')}
            <ChevronDown />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => download('json')}>
              <FileJson />
              {t('export.json')}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => download('chat')}>
              <MessagesSquare />
              {t('export.chat')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      }
    >
      <DataTable
        toolbar={
          <>
            <SearchField
              value={search.q}
              onChange={(q) => set({ q })}
              placeholder={t('search')}
            />
            <SearchField
              value={search.text}
              onChange={(text) => set({ text })}
              placeholder={t('searchConversations')}
            />
            <Choice
              aria-label={t('filters.window')}
              className="h-8 w-40"
              value={search.window}
              onChange={(v) => set({ window: parseWindow(v) })}
              options={LOG_WINDOWS.map((value) => ({
                value,
                label: windowLabels[value],
              }))}
            />
            <ListFilterPill
              label={t('filters.team')}
              {...filter('team_id')}
              options={options(teams)}
            />
            <ListFilterPill
              label={t('filters.key')}
              {...filter('key_id')}
              options={options(keys)}
            />
            <ListFilterPill
              label={t('filters.model')}
              {...filter('model')}
              options={models.map((name) => ({ value: name, label: name }))}
            />
            <ListFilterPill
              label={t('filters.label')}
              {...filter('label_id')}
              options={options(labels)}
            />
            <FilterPill
              label={t('filters.status')}
              value={search.status}
              onChange={(v) =>
                set({
                  status: v === 'success' || v === 'error' ? v : undefined,
                })
              }
              options={[
                { value: 'success', label: t('filters.success') },
                { value: 'error', label: t('filters.errors') },
              ]}
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
          </>
        }
        head={
          <>
            {sortable('created_at', t('table.time'))}
            <ColumnHead>{t('table.status')}</ColumnHead>
            <ColumnHead>{t('table.model')}</ColumnHead>
            <ColumnHead>{t('table.team')}</ColumnHead>
            <ColumnHead>{t('table.key')}</ColumnHead>
            {sortable('tokens', t('table.tokens'), 'text-right')}
            {sortable('cost', t('table.cost'), 'text-right')}
            {sortable('latency', t('table.latency'), 'text-right')}
          </>
        }
        empty={
          page.data.length === 0 &&
          (filtered ? (
            <DataTableEmpty
              title={tTable('noResults')}
              description={tTable('noResultsHint')}
            />
          ) : (
            <DataTableEmpty
              title={t('empty.title')}
              description={t('empty.description')}
            />
          ))
        }
        footer={
          <ListFooter
            list={page}
            onChange={(patch) =>
              navigate({ search: (prev) => ({ ...prev, ...patch }) })
            }
          />
        }
      >
        {page.data.map((row) => (
          <DataRow key={row.id} onOpen={() => setOpen(row)}>
            <Cell className="whitespace-nowrap text-muted-foreground">
              {money.dateTime(row.created_at)}
            </Cell>
            <Cell>
              <StatusBadge code={row.status_code} />
            </Cell>
            <Cell>
              <span className="font-mono text-xs">{row.model_name}</span>
              <p className="text-xs text-muted-foreground">
                /v1/{row.endpoint}
                {row.stream && ` · ${t('stream')}`}
              </p>
              {row.error && (
                <p className="max-w-80 truncate text-xs text-destructive">
                  {row.error}
                </p>
              )}
            </Cell>
            <Cell>{billedTo(row, t('personal'))}</Cell>
            <Cell>
              <p>{row.key_name ?? `#${row.key_id}`}</p>
              <p className="font-mono text-xs text-muted-foreground">
                {row.key_hint}
              </p>
              {row.end_user && (
                <p className="text-xs text-muted-foreground">
                  {t('forEndUser', { user: row.end_user })}
                </p>
              )}
            </Cell>
            <Cell>
              <Tokens row={row} />
            </Cell>
            <Cell className="text-right tabular-nums">
              {money.usd(row.cost_usd)}
            </Cell>
            <Cell className="text-right tabular-nums text-muted-foreground">
              {t('ms', { value: row.latency_ms })}
            </Cell>
          </DataRow>
        ))}
      </DataTable>
      <LogDetail row={open} onClose={() => setOpen(null)} />
    </Page>
  );
}
