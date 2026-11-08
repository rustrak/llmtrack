import {
  Avatar,
  Button,
  createDataTableColumnHelper,
  DataTable,
  DeleteIcon,
  NewIcon,
  Tooltip,
} from '@rustrak/ui';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
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
import { FilterPill, ListQueryBar } from '@/shared/ui/components/list-filters';
import { Page } from '@/shared/ui/components/page';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { TeamsIcon } from '@/shared/ui/icons';

/** What `GET /api/people` sorts by when not told. */
const DEFAULT_SORT = 'name';
const column = createDataTableColumnHelper<Person>();

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
  const teamOptions = teams.map((x) => ({
    value: String(x.id),
    label: x.name,
  }));

  const columns = useMemo(
    () =>
      column.columns([
        column.accessor('name', {
          id: 'name',
          header: t('table.name'),
          enableHiding: false,
          cell: ({ row }) => {
            const person = row.original;
            return (
              <div className="flex min-w-0 items-center gap-3 py-1.5">
                <Avatar name={person.name} />
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="truncate text-value font-medium text-fg">
                    {person.name}
                  </span>
                  {person.email && (
                    <span className="truncate text-hint text-fg-subtle">
                      {person.email}
                    </span>
                  )}
                </div>
              </div>
            );
          },
          meta: { label: t('table.name') },
        }),
        column.accessor('team_name', {
          id: 'team',
          header: t('table.team'),
          cell: ({ row }) =>
            row.original.team_id === null ? (
              <span className="text-fg-subtle">{t('noTeam')}</span>
            ) : (
              <Link
                to="/teams/$teamId"
                params={{ teamId: String(row.original.team_id) }}
                className="text-fg hover:underline"
              >
                {row.original.team_name}
              </Link>
            ),
          meta: { label: t('table.team'), width: 180 },
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
        column.accessor('spend_usd', {
          id: 'spend',
          header: t('table.spend'),
          cell: ({ getValue }) => (
            <span className="font-mono text-mono-sm tabular-nums">
              {money.usd(getValue())}
            </span>
          ),
          meta: {
            label: t('table.spend'),
            width: 128,
            align: 'end',
            numeric: true,
          },
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
        column.display({
          id: 'actions',
          header: '',
          enableSorting: false,
          enableHiding: false,
          cell: ({ row }) => {
            const person = row.original;
            return (
              canManage(person) && (
                <Tooltip content={t('delete.label', { name: person.name })}>
                  <Button
                    variant="ghost"
                    size="sm"
                    icon={DeleteIcon}
                    aria-label={t('delete.label', { name: person.name })}
                    onClick={() => setDeleting(person)}
                  />
                </Tooltip>
              )
            );
          },
          meta: { width: 56, align: 'end' },
        }),
      ]),
    [t, money, canManage],
  );

  const table = useListTable({
    data: people.data,
    columns,
    total: people.total,
    params: search,
    defaultSort: DEFAULT_SORT,
    onParams: (patch) =>
      navigate({ search: (prev) => ({ ...prev, ...patch }) }),
    getRowId: (person) => String(person.id),
    storageKey: 'people',
  });

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        (isAdmin || managed.length > 0) && (
          <Button
            variant="primary"
            icon={NewIcon}
            onClick={() => setCreating(true)}
          >
            {t('create.button')}
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
            onChange={(patch) => set(patch as PeopleListParams)}
            fields={[
              {
                key: 'team_id',
                label: t('table.team'),
                icon: TeamsIcon,
                variant: 'options',
                multiple: false,
                options: teamOptions,
              },
            ]}
          />
          <FilterPill
            label={t('table.team')}
            value={search.team_id ? String(search.team_id) : undefined}
            onChange={(v) => set({ team_id: v ? Number(v) : undefined })}
            options={teamOptions}
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
                  description: t('empty.description'),
                }
          }
        />
      </div>
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
