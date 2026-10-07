import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { Plus } from 'lucide-react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { listModels } from '@/features/model/api/queries';
import { listTeams, type TeamListParams } from '@/features/team/api/queries';
import { TeamPanel } from '@/features/team/ui/components/team-panel';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { MAX_PAGE_SIZE, parseListParams } from '@/shared/lib/list-params';
import {
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
  FilterPill,
  ListFooter,
  SearchField,
  SortHead,
} from '@/shared/ui/components/data-table';
import { Page } from '@/shared/ui/components/page';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { Button } from '@/shared/ui/components/shadcn/button';
import { SpendBar } from '@/shared/ui/components/spend-bar';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useMoney } from '@/shared/ui/hooks/use-money';

const ACCESS = ['all', 'restricted'] as const;
/** What `GET /api/teams` sorts by when not told. */
const DEFAULT_SORT = 'name';

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
        isAdmin && (
          <Button onClick={() => setCreating(true)}>
            <Plus />
            {t('create')}
          </Button>
        )
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
            <FilterPill
              label={t('filters.models')}
              value={search.models}
              onChange={(v) => set({ models: ACCESS.find((a) => a === v) })}
              options={ACCESS.map((value) => ({
                value,
                label: t(`access.${value}`),
              }))}
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
          </>
        }
        head={
          <>
            {sortable('name', t('table.team'), 'min-w-48')}
            {sortable('spend', t('table.spend'), 'min-w-40')}
            <ColumnHead>{t('table.models')}</ColumnHead>
            {sortable('members', t('table.members'))}
            {sortable('keys', t('table.keys'))}
            <ColumnHead>{t('table.yourRole')}</ColumnHead>
            {sortable('created_at', t('table.created'))}
          </>
        }
        empty={
          teams.data.length === 0 &&
          (filtered ? (
            <DataTableEmpty
              title={tTable('noResults')}
              description={tTable('noResultsHint')}
            />
          ) : (
            <DataTableEmpty
              title={t('empty.title')}
              description={isAdmin ? t('empty.admin') : t('empty.member')}
            />
          ))
        }
        footer={
          <ListFooter
            list={teams}
            onChange={(patch) =>
              navigate({ search: (prev) => ({ ...prev, ...patch }) })
            }
          />
        }
      >
        {teams.data.map((team) => (
          <DataRow
            key={team.id}
            onOpen={() =>
              navigate({
                to: '/teams/$teamId',
                params: { teamId: String(team.id) },
              })
            }
          >
            <Cell>
              <Link
                to="/teams/$teamId"
                params={{ teamId: String(team.id) }}
                className="font-medium underline-offset-4 hover:underline"
              >
                {team.name}
              </Link>
            </Cell>
            <Cell>
              <SpendBar spent={team.spend_usd} budget={team.max_budget_usd} />
            </Cell>
            <Cell className="text-muted-foreground">
              {team.all_models
                ? t('allModels')
                : t('modelCount', { count: team.models.length })}
            </Cell>
            <Cell className="tabular-nums">{team.member_count}</Cell>
            <Cell className="tabular-nums">{team.key_count}</Cell>
            <Cell>
              {team.my_role ? (
                <Badge variant="outline">{tRoles(team.my_role)}</Badge>
              ) : (
                <span className="text-muted-foreground">—</span>
              )}
            </Cell>
            <Cell className="whitespace-nowrap text-muted-foreground">
              {money.date(team.created_at)}
            </Cell>
          </DataRow>
        ))}
      </DataTable>
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
