import {
  Button,
  createDataTableColumnHelper,
  DataTable,
  NewIcon,
  Tag,
} from '@rustrak/ui';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import { useTranslations } from 'use-intl';
import { listModels } from '@/features/model/api/queries';
import { listTeams, type TeamListParams } from '@/features/team/api/queries';
import { TeamPanel } from '@/features/team/ui/components/team-panel';
import { unwrap } from '@/shared/api/http';
import type { Team } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import { MAX_PAGE_SIZE, parseListParams } from '@/shared/lib/list-params';
import { FilterPill, ListQueryBar } from '@/shared/ui/components/list-filters';
import { Page } from '@/shared/ui/components/page';
import { SpendBar } from '@/shared/ui/components/spend-bar';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';

const ACCESS = ['all', 'restricted'] as const;
/** What `GET /api/teams` sorts by when not told. */
const DEFAULT_SORT = 'name';
const column = createDataTableColumnHelper<Team>();

export const Route = createFileRoute('/_authenticated/teams/')({
  validateSearch: (search: Record<string, unknown>): TeamListParams => {
    const models = ACCESS.find((a) => a === search.models);
    return { ...parseListParams(search), ...(models ? { models } : {}) };
  },
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => {
    const [teams, models] = await Promise.all([
      listTeams(deps),
      listModels({ per_page: MAX_PAGE_SIZE }),
    ]);
    return { teams: unwrap(teams), models: unwrap(models).data };
  },
  head: () => ({ meta: [{ title: translator('teams')('meta.title') }] }),
  component: TeamsPage,
});

function TeamsPage() {
  const t = useTranslations('teams');
  const tRoles = useTranslations('roles');
  const tTable = useTranslations('table');
  const { teams, models } = Route.useLoaderData();
  const search = Route.useSearch();
  const isAdmin = useCurrentUser().role === 'admin';
  const navigate = useNavigate({ from: Route.fullPath });
  const money = useMoney();
  const [creating, setCreating] = useState(false);
  const set = (patch: TeamListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(search.q || search.models);
  const accessOptions = ACCESS.map((value) => ({
    value,
    label: t(`access.${value}`),
  }));

  const columns = useMemo(
    () =>
      column.columns([
        column.accessor('name', {
          id: 'name',
          header: t('table.team'),
          enableHiding: false,
          cell: ({ getValue }) => (
            <span className="truncate text-value font-medium text-fg">
              {getValue()}
            </span>
          ),
          meta: { label: t('table.team') },
        }),
        column.accessor('spend_usd', {
          id: 'spend',
          header: t('table.spend'),
          cell: ({ row }) => (
            <SpendBar
              spent={row.original.spend_usd}
              budget={row.original.max_budget_usd}
            />
          ),
          meta: { label: t('table.spend'), width: 180 },
        }),
        column.accessor('all_models', {
          id: 'models',
          header: t('table.models'),
          enableSorting: false,
          cell: ({ row }) => (
            <span className="text-fg-subtle">
              {row.original.all_models
                ? t('allModels')
                : t('modelCount', { count: row.original.models.length })}
            </span>
          ),
          meta: { label: t('table.models'), width: 160 },
        }),
        column.accessor('member_count', {
          id: 'members',
          header: t('table.members'),
          cell: ({ getValue }) => (
            <span className="font-mono text-mono-sm tabular-nums">
              {getValue()}
            </span>
          ),
          meta: {
            label: t('table.members'),
            width: 112,
            align: 'end',
            numeric: true,
          },
        }),
        column.accessor('key_count', {
          id: 'keys',
          header: t('table.keys'),
          cell: ({ getValue }) => (
            <span className="font-mono text-mono-sm tabular-nums">
              {getValue()}
            </span>
          ),
          meta: {
            label: t('table.keys'),
            width: 96,
            align: 'end',
            numeric: true,
          },
        }),
        column.accessor('my_role', {
          id: 'my_role',
          header: t('table.yourRole'),
          enableSorting: false,
          cell: ({ getValue }) => {
            const role = getValue();
            return role ? (
              <Tag tone={role === 'admin' ? 'brand' : 'neutral'}>
                {tRoles(role)}
              </Tag>
            ) : (
              <span className="text-fg-subtle">—</span>
            );
          },
          meta: { label: t('table.yourRole'), width: 128 },
        }),
        column.accessor('created_at', {
          id: 'created_at',
          header: t('table.created'),
          cell: ({ getValue }) => (
            <span className="whitespace-nowrap text-fg-subtle">
              {money.date(getValue())}
            </span>
          ),
          meta: { label: t('table.created'), width: 128 },
        }),
      ]),
    [t, tRoles, money],
  );

  const table = useListTable({
    data: teams.data,
    columns,
    total: teams.total,
    params: search,
    defaultSort: DEFAULT_SORT,
    onParams: (patch) =>
      navigate({ search: (prev) => ({ ...prev, ...patch }) }),
    getRowId: (team) => String(team.id),
    storageKey: 'teams',
  });

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        isAdmin && (
          <Button
            variant="primary"
            icon={NewIcon}
            onClick={() => setCreating(true)}
          >
            {t('create')}
          </Button>
        )
      }
    >
      <div className="flex min-h-0 flex-1 flex-col gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <ListQueryBar
            className="min-w-0 flex-1 sm:max-w-md max-sm:basis-full"
            placeholder={t('search')}
            search={search}
            onChange={(patch) => set(patch as TeamListParams)}
            fields={[
              {
                key: 'models',
                label: t('filters.models'),
                variant: 'options',
                multiple: false,
                options: accessOptions,
              },
            ]}
          />
          <FilterPill
            label={t('filters.models')}
            value={search.models}
            onChange={(v) => set({ models: ACCESS.find((a) => a === v) })}
            options={accessOptions}
          />
          {filtered && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => set({ q: undefined, models: undefined })}
            >
              {tTable('clearFilters')}
            </Button>
          )}
        </div>
        <DataTable
          table={table}
          className="min-h-0 flex-1"
          empty={
            filtered
              ? {
                  title: tTable('noResults'),
                  description: tTable('noResultsHint'),
                }
              : {
                  title: t('empty.title'),
                  description: isAdmin ? t('empty.admin') : t('empty.member'),
                }
          }
          onRowClick={(row) =>
            navigate({
              to: '/teams/$teamId',
              params: { teamId: String(row.original.id) },
            })
          }
        />
      </div>
      <TeamPanel
        open={creating}
        models={models}
        onClose={() => setCreating(false)}
        onSaved={(id) =>
          navigate({ to: '/teams/$teamId', params: { teamId: String(id) } })
        }
      />
    </Page>
  );
}
