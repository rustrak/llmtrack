import { Button, NewIcon, Tab, TabList, Tabs } from '@rustrak/ui';
import {
  createFileRoute,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';
import { useCallback, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { deleteModel, updateModel } from '@/features/model/api/mutations';
import {
  getRouterSettings,
  listModels,
  listProviders,
  type ModelListParams,
} from '@/features/model/api/queries';
import { ModelPanel } from '@/features/model/ui/components/model-panel';
import { ModelsTable } from '@/features/model/ui/components/models-table';
import { RouterSettingsForm } from '@/features/model/ui/components/router-settings';
import { useConnectionTest } from '@/features/model/ui/components/use-connection-test';
import { unwrap } from '@/shared/api/http';
import type { Model } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { MAX_PAGE_SIZE, parseListParams } from '@/shared/lib/list-params';
import { ConfirmDialog } from '@/shared/ui/components/confirm-dialog';
import { FilterPill, ListQueryBar } from '@/shared/ui/components/list-filters';
import { Page } from '@/shared/ui/components/page';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';

const STATUSES = ['active', 'inactive'] as const;

export const Route = createFileRoute('/_authenticated/models')({
  validateSearch: (search: Record<string, unknown>): ModelListParams => {
    const status = STATUSES.find((s) => s === search.status);
    return {
      ...parseListParams(search),
      ...(typeof search.provider === 'string' && search.provider
        ? { provider: search.provider }
        : {}),
      ...(status ? { status } : {}),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => {
    const [models, providers, routerSettings] = await Promise.all([
      listModels(deps),
      listProviders(),
      getRouterSettings(),
    ]);
    // Admins only; anyone else simply does not see the tab.
    const names = routerSettings.success
      ? unwrap(await listModels({ per_page: MAX_PAGE_SIZE })).data.map(
          (m) => m.name,
        )
      : [];
    return {
      models: unwrap(models),
      providers: unwrap(providers),
      routerSettings: routerSettings.success ? routerSettings.data : null,
      names: [...new Set(names)],
    };
  },
  head: () => ({ meta: [{ title: translator('models')('meta.title') }] }),
  component: ModelsPage,
});

function ModelsPage() {
  const t = useTranslations('models');
  const tTable = useTranslations('table');
  const { models, providers, routerSettings, names } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const set = (patch: ModelListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(search.q || search.provider || search.status);
  const isAdmin = useCurrentUser().role === 'admin';
  const router = useRouter();
  // Which model the panel shows stays set while it slides out.
  const [editing, setEditingModel] = useState<Model | undefined>();
  const [panelOpen, setPanelOpen] = useState(false);
  // Stable, like everything handed to the table: a new function each render
  // rebuilds its columns, and their row menus with them.
  const setEditing = useCallback((model: Model | 'new') => {
    setEditingModel(model === 'new' ? undefined : model);
    setPanelOpen(true);
  }, []);
  const [deleting, setDeleting] = useState<Model | null>(null);
  const [tab, setTab] = useState<'models' | 'router'>('models');
  const label = useCallback(
    (id: string) => providers.find((p) => p.id === id)?.label ?? id,
    [providers],
  );
  const providerOptions = providers.map((p) => ({
    value: p.id,
    label: p.label,
  }));
  const testConnection = useConnectionTest();
  const test = useCallback(
    (model: Model) =>
      testConnection(model.name, {
        model_id: model.id,
        provider: model.provider,
        upstream_model: model.upstream_model,
        api_base: model.api_base,
        api_version: model.api_version,
      }),
    [testConnection],
  );
  const toggle = useCallback(
    async (model: Model, is_active: boolean) => {
      const result = await updateModel(model.id, { is_active });
      if (!result.success) toast.error(readableMessage(result.error));
      await router.invalidate();
    },
    [router],
  );

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
            onClick={() => setEditing('new')}
          >
            {t('add')}
          </Button>
        )
      }
      tabs={
        routerSettings && (
          <Tabs
            value={tab}
            onValueChange={(next) => setTab(next as 'models' | 'router')}
          >
            <TabList>
              <Tab value="models">{t('tabs.models')}</Tab>
              <Tab value="router">{t('tabs.router')}</Tab>
            </TabList>
          </Tabs>
        )
      }
    >
      {tab === 'models' || !routerSettings ? (
        <div className="flex min-h-0 flex-1 flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <ListQueryBar
              className="min-w-0 flex-1 sm:max-w-md max-sm:basis-full"
              placeholder={t('search')}
              search={search}
              onChange={(patch) => set(patch as ModelListParams)}
              fields={[
                {
                  key: 'provider',
                  label: t('filters.provider'),
                  variant: 'options',
                  multiple: false,
                  options: providerOptions,
                },
                {
                  key: 'status',
                  label: t('filters.status'),
                  variant: 'options',
                  multiple: false,
                  options: STATUSES.map((value) => ({
                    value,
                    label: t(`statusFilter.${value}`),
                    tone: value === 'active' ? 'brand' : 'error',
                  })),
                },
              ]}
            />
            <FilterPill
              label={t('filters.provider')}
              value={search.provider}
              onChange={(provider) => set({ provider })}
              options={providerOptions}
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
                  set({ q: undefined, provider: undefined, status: undefined })
                }
              >
                {tTable('clearFilters')}
              </Button>
            )}
            <p className="ml-auto text-meta text-fg-subtle">
              {t('pricesPerMillion')}
            </p>
          </div>
          <ModelsTable
            models={models.data}
            total={models.total}
            params={search}
            onParams={(patch) =>
              navigate({ search: (prev) => ({ ...prev, ...patch }) })
            }
            providerLabel={label}
            isAdmin={isAdmin}
            onToggle={toggle}
            onEdit={setEditing}
            onTest={test}
            onDelete={setDeleting}
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
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto">
          <RouterSettingsForm settings={routerSettings} names={names} />
        </div>
      )}
      <ModelPanel
        open={panelOpen}
        model={editing}
        providers={providers}
        onClose={() => setPanelOpen(false)}
        onSaved={() => router.invalidate()}
      />
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title={t('deleteDialog.title', { name: deleting?.name ?? '' })}
        description={t('deleteDialog.description')}
        confirmLabel={t('deleteDialog.confirm')}
        onConfirm={async () => {
          if (!deleting) return null;
          const result = await deleteModel(deleting.id);
          if (!result.success) return readableMessage(result.error);
          await router.invalidate();
          return null;
        }}
      />
    </Page>
  );
}
