import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import {
  type KeyListParams,
  type KeyStatusFilter,
  listKeys,
} from '@/features/key/api/queries';
import {
  CreateKeyButton,
  KeyManager,
} from '@/features/key/ui/components/key-manager';
import { listLabels } from '@/features/label/api/queries';
import { listModels } from '@/features/model/api/queries';
import { getSettings } from '@/features/settings/api/queries';
import { listPeople, listTeams } from '@/features/team/api/queries';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { MAX_PAGE_SIZE, parseListParams } from '@/shared/lib/list-params';
import {
  FilterPill,
  ListFooter,
  SearchField,
} from '@/shared/ui/components/data-table';
import { Page } from '@/shared/ui/components/page';
import { Button } from '@/shared/ui/components/shadcn/button';

const STATUSES: KeyStatusFilter[] = ['active', 'blocked', 'expired'];

export const Route = createFileRoute('/_authenticated/keys/')({
  validateSearch: (search: Record<string, unknown>): KeyListParams => {
    const team = Number(search.team_id);
    const status = STATUSES.find((s) => s === search.status);
    return {
      ...parseListParams(search),
      ...(Number.isInteger(team) && team > 0 ? { team_id: team } : {}),
      ...(status ? { status } : {}),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => {
    const [keys, teams, people, models, labels, settings] = await Promise.all([
      listKeys(deps),
      listTeams({ per_page: MAX_PAGE_SIZE }),
      // ponytail: the first 200 people; page or search them if teams get bigger.
      listPeople({ per_page: MAX_PAGE_SIZE }),
      listModels({ per_page: MAX_PAGE_SIZE }),
      listLabels({ per_page: MAX_PAGE_SIZE }),
      getSettings(),
    ]);
    const everyone = unwrap(people).data;
    return {
      keys: unwrap(keys),
      // Each team with its people, for the key form's "Person".
      teams: unwrap(teams).data.map((team) => ({
        ...team,
        people: everyone.filter((p) => p.team_id === team.id),
      })),
      models: unwrap(models).data,
      labels: unwrap(labels).data,
      publicUrl: unwrap(settings).public_url,
    };
  },
  head: () => ({ meta: [{ title: translator('keys')('meta.title') }] }),
  component: KeysPage,
});

function KeysPage() {
  const t = useTranslations('keys');
  const tTable = useTranslations('table');
  const { keys, teams, models, labels, publicUrl } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const allModels = models.map(({ id, name }) => ({ id, name }));
  // Any change but the page itself starts again from the first page.
  const set = (patch: KeyListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(search.q || search.team_id || search.status);

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        <CreateKeyButton
          teams={teams}
          allModels={allModels}
          labels={labels}
          teamId={search.team_id}
          publicUrl={publicUrl}
        />
      }
    >
      <KeyManager
        keys={keys.data}
        teams={teams}
        allModels={allModels}
        labels={labels}
        teamId={search.team_id}
        publicUrl={publicUrl}
        sort={search.sort}
        onSort={(sort) => set({ sort })}
        toolbar={
          <>
            <SearchField
              value={search.q}
              onChange={(q) => set({ q })}
              placeholder={t('search')}
            />
            <FilterPill
              label={t('filters.team')}
              value={search.team_id ? String(search.team_id) : undefined}
              onChange={(v) => set({ team_id: v ? Number(v) : undefined })}
              options={teams.map((x) => ({
                value: String(x.id),
                label: x.name,
              }))}
            />
            <FilterPill
              label={t('filters.status')}
              value={search.status}
              onChange={(v) => set({ status: STATUSES.find((s) => s === v) })}
              options={STATUSES.map((value) => ({
                value,
                label: t(`statusFilter.${value}`),
              }))}
            />
            {filtered && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  set({ q: undefined, team_id: undefined, status: undefined })
                }
              >
                {tTable('clearFilters')}
              </Button>
            )}
          </>
        }
        empty={
          filtered
            ? {
                title: tTable('noResults'),
                description: tTable('noResultsHint'),
              }
            : { title: t('empty.title'), description: t('empty.description') }
        }
        footer={
          <ListFooter
            list={keys}
            onChange={(patch) =>
              navigate({ search: (prev) => ({ ...prev, ...patch }) })
            }
          />
        }
      />
    </Page>
  );
}
