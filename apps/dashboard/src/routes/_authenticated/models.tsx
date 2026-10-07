import {
  createFileRoute,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';
import { AlertTriangle, MoreHorizontal, Plus } from 'lucide-react';
import { useState } from 'react';
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
import { RouterSettingsForm } from '@/features/model/ui/components/router-settings';
import { useConnectionTest } from '@/features/model/ui/components/use-connection-test';
import { unwrap } from '@/shared/api/http';
import type { Model } from '@/shared/api/schemas';
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
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/components/shadcn/dropdown-menu';
import { Switch } from '@/shared/ui/components/shadcn/switch';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useMoney } from '@/shared/ui/hooks/use-money';

const STATUSES = ['active', 'inactive'] as const;
/** What `GET /api/models` sorts by when not told. */
const DEFAULT_SORT = 'name';

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

/** Where a model's price comes from, with the price-list entry it uses. */
function PricingSource({ model }: { model: Model }) {
  const tPricing = useTranslations('pricing');
  switch (model.pricing_source) {
    case 'none':
      return (
        <Badge variant="destructive" title={tPricing('source.noneHint')}>
          <AlertTriangle />
          {tPricing('source.none')}
        </Badge>
      );
    case 'free':
      return (
        <Badge variant="secondary" title={tPricing('source.freeHint')}>
          {tPricing('source.free')}
        </Badge>
      );
    case 'custom':
      return <Badge variant="secondary">{tPricing('source.custom')}</Badge>;
    default:
      return (
        <div className="space-y-1">
          <Badge variant="outline">{tPricing('source.catalog')}</Badge>
          <p className="max-w-40 truncate font-mono text-xs text-muted-foreground">
            {model.catalog_key}
          </p>
        </div>
      );
  }
}

function ModelsPage() {
  const t = useTranslations('models');
  const tTable = useTranslations('table');
  const { models, providers, routerSettings, names } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const set = (patch: ModelListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(search.q || search.provider || search.status);
  const sortable = (field: string, label: string) => (
    <SortHead
      field={field}
      sort={search.sort}
      fallback={DEFAULT_SORT}
      onSort={(sort) => set({ sort })}
    >
      {label}
    </SortHead>
  );
  const isAdmin = useCurrentUser().role === 'admin';
  const router = useRouter();
  const money = useMoney();
  // Which model the panel shows stays set while it slides out.
  const [editing, setEditingModel] = useState<Model | undefined>();
  const [panelOpen, setPanelOpen] = useState(false);
  const setEditing = (model: Model | 'new') => {
    setEditingModel(model === 'new' ? undefined : model);
    setPanelOpen(true);
  };
  const [deleting, setDeleting] = useState<Model | null>(null);
  const label = (id: string) => providers.find((p) => p.id === id)?.label ?? id;
  const testConnection = useConnectionTest();
  const test = (model: Model) =>
    testConnection(model.name, {
      model_id: model.id,
      provider: model.provider,
      upstream_model: model.upstream_model,
      api_base: model.api_base,
      api_version: model.api_version,
    });
  const price = (value: number | undefined) =>
    value === undefined ? '—' : money.usd(value);

  const toggle = async (model: Model, is_active: boolean) => {
    const result = await updateModel(model.id, { is_active });
    if (!result.success) toast.error(readableMessage(result.error));
    await router.invalidate();
  };

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        isAdmin && (
          <Button onClick={() => setEditing('new')}>
            <Plus />
            {t('add')}
          </Button>
        )
      }
    >
      <Tabs defaultValue="models" className="min-h-0 flex-1">
        {routerSettings && (
          <TabsList>
            <TabsTrigger value="models">{t('tabs.models')}</TabsTrigger>
            <TabsTrigger value="router">{t('tabs.router')}</TabsTrigger>
          </TabsList>
        )}
        <TabsContent value="models" className="flex min-h-0 flex-col pt-2">
          <DataTable
            toolbar={
              <>
                <SearchField
                  value={search.q}
                  onChange={(q) => set({ q })}
                  placeholder={t('search')}
                />
                <FilterPill
                  label={t('filters.provider')}
                  value={search.provider}
                  onChange={(provider) => set({ provider })}
                  options={providers.map((p) => ({
                    value: p.id,
                    label: p.label,
                  }))}
                />
                <FilterPill
                  label={t('filters.status')}
                  value={search.status}
                  onChange={(v) =>
                    set({ status: STATUSES.find((s) => s === v) })
                  }
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
                      set({
                        q: undefined,
                        provider: undefined,
                        status: undefined,
                      })
                    }
                  >
                    {tTable('clearFilters')}
                  </Button>
                )}
                <p className="ml-auto text-xs text-muted-foreground">
                  {t('pricesPerMillion')}
                </p>
              </>
            }
            head={
              <>
                {sortable('name', t('table.name'))}
                {sortable('provider', t('table.provider'))}
                {sortable('upstream_model', t('table.upstream'))}
                <ColumnHead>{t('table.pricing')}</ColumnHead>
                <ColumnHead className="text-right">
                  {t('table.input')}
                </ColumnHead>
                <ColumnHead className="text-right">
                  {t('table.output')}
                </ColumnHead>
                <ColumnHead className="text-right">
                  {t('table.cacheRead')}
                </ColumnHead>
                <ColumnHead>{t('table.key')}</ColumnHead>
                <ColumnHead>{t('table.active')}</ColumnHead>
                {isAdmin && <ColumnHead className="w-12" />}
              </>
            }
            empty={
              models.data.length === 0 &&
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
                list={models}
                onChange={(patch) =>
                  navigate({ search: (prev) => ({ ...prev, ...patch }) })
                }
              />
            }
          >
            {models.data.map((model) => (
              <DataRow
                key={model.id}
                className={model.is_active ? undefined : 'opacity-60'}
                onOpen={isAdmin ? () => setEditing(model) : undefined}
              >
                <Cell className="font-mono font-medium">
                  {model.name}
                  {model.deployments > 1 && (
                    <p className="font-sans text-xs font-normal text-muted-foreground">
                      {model.weight === null
                        ? t('deployment')
                        : t('deploymentWeight', { weight: model.weight })}
                    </p>
                  )}
                </Cell>
                <Cell>
                  <Badge variant="secondary">{label(model.provider)}</Badge>
                </Cell>
                <Cell>
                  <p className="font-mono text-xs">{model.upstream_model}</p>
                  <p className="max-w-56 truncate font-mono text-xs text-muted-foreground">
                    {model.effective_api_base}
                  </p>
                </Cell>
                <Cell>
                  <PricingSource model={model} />
                </Cell>
                <Cell className="text-right tabular-nums">
                  {price(model.pricing?.input)}
                </Cell>
                <Cell className="text-right tabular-nums">
                  {price(model.pricing?.output)}
                </Cell>
                <Cell className="text-right tabular-nums text-muted-foreground">
                  {price(model.pricing?.cache_read)}
                </Cell>
                <Cell>
                  {model.has_api_key ? (
                    <Badge variant="outline">{t('keyStored')}</Badge>
                  ) : (
                    <span className="text-sm text-muted-foreground">
                      {t('keyNone')}
                    </span>
                  )}
                </Cell>
                <Cell>
                  <Switch
                    checked={model.is_active}
                    disabled={!isAdmin}
                    onCheckedChange={(next) => toggle(model, next)}
                    aria-label={t('activeLabel', { name: model.name })}
                  />
                </Cell>
                {isAdmin && (
                  <ActionsCell>
                    <DropdownMenu>
                      <DropdownMenuTrigger
                        render={
                          <Button
                            variant="ghost"
                            size="icon-sm"
                            aria-label={t('actionsFor', { name: model.name })}
                          />
                        }
                      >
                        <MoreHorizontal />
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => setEditing(model)}>
                          {t('edit')}
                        </DropdownMenuItem>
                        <DropdownMenuItem onClick={() => test(model)}>
                          {t('test.button')}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          className="text-destructive"
                          onClick={() => setDeleting(model)}
                        >
                          {t('delete')}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </ActionsCell>
                )}
              </DataRow>
            ))}
          </DataTable>
        </TabsContent>
        {routerSettings && (
          <TabsContent value="router" className="min-h-0 overflow-y-auto pt-2">
            <RouterSettingsForm settings={routerSettings} names={names} />
          </TabsContent>
        )}
      </Tabs>
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
