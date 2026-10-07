import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { deletePerson } from '@/features/team/api/mutations';
import {
  listPeople,
  listTeams,
  type PeopleListParams,
} from '@/features/team/api/queries';
import { PersonPanel } from '@/features/team/ui/components/person-panel';
import { unwrap } from '@/shared/api/http';
import type { Person } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { MAX_PAGE_SIZE, parseListParams } from '@/shared/lib/list-params';
import { ConfirmDialog } from '@/shared/ui/components/confirm-dialog';
import {
  ActionsCell,
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
import { Button } from '@/shared/ui/components/shadcn/button';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** What `GET /api/people` sorts by when not told. */
const DEFAULT_SORT = 'name';

export const Route = createFileRoute('/_authenticated/people')({
  validateSearch: (search: Record<string, unknown>): PeopleListParams => {
    const team = Number(search.team_id);
    return {
      ...parseListParams(search),
      ...(Number.isInteger(team) && team > 0 ? { team_id: team } : {}),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => {
    const [people, teams] = await Promise.all([
      listPeople(deps),
      listTeams({ per_page: MAX_PAGE_SIZE }),
    ]);
    return { people: unwrap(people), teams: unwrap(teams).data };
  },
  head: () => ({ meta: [{ title: translator('people')('meta.title') }] }),
  component: PeoplePage,
});

function PeoplePage() {
  const t = useTranslations('people');
  const tTable = useTranslations('table');
  const { people, teams } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const money = useMoney();
  const isAdmin = useCurrentUser().role === 'admin';
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState<Person | null>(null);
  // Only a team's admins add its people; only admins keep them teamless.
  const managed = teams.filter((x) => isAdmin || x.my_role === 'admin');
  const canManage = (person: Person) =>
    isAdmin || managed.some((x) => x.id === person.team_id);
  const set = (patch: PeopleListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(search.q || search.team_id);
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
        (isAdmin || managed.length > 0) && (
          <Button onClick={() => setCreating(true)}>
            <Plus />
            {t('create.button')}
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
              label={t('table.team')}
              value={search.team_id ? String(search.team_id) : undefined}
              onChange={(v) => set({ team_id: v ? Number(v) : undefined })}
              options={teams.map((x) => ({
                value: String(x.id),
                label: x.name,
              }))}
            />
            {filtered && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => set({ q: undefined, team_id: undefined })}
              >
                {tTable('clearFilters')}
              </Button>
            )}
          </>
        }
        head={
          <>
            {sortable('name', t('table.name'), 'min-w-48')}
            {sortable('team', t('table.team'), 'min-w-40')}
            {sortable('keys', t('table.keys'))}
            {sortable('spend', t('table.spend'))}
            {sortable('created_at', t('table.created'))}
            <ColumnHead className="w-10" />
          </>
        }
        empty={
          people.data.length === 0 &&
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
            list={people}
            onChange={(patch) =>
              navigate({ search: (prev) => ({ ...prev, ...patch }) })
            }
          />
        }
      >
        {people.data.map((person) => (
          <DataRow key={person.id}>
            <Cell>
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="truncate font-medium">{person.name}</span>
                {person.email && (
                  <span className="truncate text-xs text-muted-foreground">
                    {person.email}
                  </span>
                )}
              </div>
            </Cell>
            <Cell>
              {person.team_id === null ? (
                <span className="text-muted-foreground">{t('noTeam')}</span>
              ) : (
                <Link
                  to="/teams/$teamId"
                  params={{ teamId: String(person.team_id) }}
                  className="underline-offset-4 hover:underline"
                >
                  {person.team_name}
                </Link>
              )}
            </Cell>
            <Cell className="tabular-nums">{person.key_count}</Cell>
            <Cell className="tabular-nums">{money.usd(person.spend_usd)}</Cell>
            <Cell className="whitespace-nowrap text-muted-foreground">
              {money.date(person.created_at)}
            </Cell>
            <ActionsCell>
              {canManage(person) && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t('delete.label', { name: person.name })}
                  title={t('delete.label', { name: person.name })}
                  onClick={() => setDeleting(person)}
                >
                  <Trash2 />
                </Button>
              )}
            </ActionsCell>
          </DataRow>
        ))}
      </DataTable>
      <PersonPanel
        open={creating}
        teams={managed}
        allowNoTeam={isAdmin}
        defaultTeamId={search.team_id}
        onClose={() => setCreating(false)}
        onSaved={() => navigate({ search: (prev) => prev })}
      />
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title={t('delete.title', { name: deleting?.name ?? '' })}
        description={t('delete.description')}
        confirmLabel={t('delete.confirm')}
        onConfirm={async () => {
          if (!deleting) return null;
          const result = await deletePerson(deleting.id);
          if (!result.success) return readableMessage(result.error);
          await navigate({ search: (prev) => prev });
          return null;
        }}
      />
    </Page>
  );
}
