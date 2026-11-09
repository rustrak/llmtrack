import {
  createDataTableColumnHelper,
  DataTable,
  DeleteIcon,
  EditIcon,
  type MenuAction,
  Switch,
  Tag,
  TestIcon,
  Tooltip,
} from '@rustrak/ui';
import { useMemo } from 'react';
import { useTranslations } from 'use-intl';
import type { Model } from '@/shared/api/schemas';
import type { ListParams } from '@/shared/lib/list-params';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** What `GET /api/models` sorts by when not told. */
export const MODELS_DEFAULT_SORT = 'name';

/** Where a model's price comes from, with the price-list entry it uses. */
function PricingSource({ model }: { model: Model }) {
  const tPricing = useTranslations('pricing');
  switch (model.pricing_source) {
    case 'none':
      return (
        <Tooltip content={tPricing('source.noneHint')}>
          <span>
            <Tag tone="error">{tPricing('source.none')}</Tag>
          </span>
        </Tooltip>
      );
    case 'free':
      return (
        <Tooltip content={tPricing('source.freeHint')}>
          <span>
            <Tag tone="info">{tPricing('source.free')}</Tag>
          </span>
        </Tooltip>
      );
    case 'custom':
      return <Tag tone="warning">{tPricing('source.custom')}</Tag>;
    default:
      return (
        <div className="space-y-0.5 py-1.5">
          <Tag tone="brand">{tPricing('source.catalog')}</Tag>
          <p className="max-w-40 truncate font-mono text-mono-sm text-fg-subtle">
            {model.catalog_key}
          </p>
        </div>
      );
  }
}

/** A model as the list returns it: with how many deployments share its name. */
type ListedModel = Model & { deployments: number };

const column = createDataTableColumnHelper<ListedModel>();

/**
 * The models as the design system's table: server-paged and server-sorted,
 * the column ids being the API's sort names. An admin opens a row to edit it
 * and gets its actions in the row menu.
 */
export function ModelsTable({
  models,
  total,
  params,
  onParams,
  providerLabel,
  isAdmin,
  onToggle,
  onEdit,
  onTest,
  onDelete,
  empty,
}: {
  models: ListedModel[];
  total: number;
  params: ListParams;
  onParams: (patch: Partial<ListParams>) => void;
  providerLabel: (id: string) => string;
  isAdmin: boolean;
  onToggle: (model: Model, active: boolean) => void;
  onEdit: (model: Model) => void;
  onTest: (model: Model) => void;
  onDelete: (model: Model) => void;
  empty: { title: string; description?: string };
}) {
  const t = useTranslations('models');
  const money = useMoney();
  const price = (value: number | undefined) =>
    value === undefined ? '—' : money.usd(value);
  const priceColumn = (
    id: 'input' | 'output' | 'cache_read',
    label: string,
    subtle = false,
  ) =>
    column.display({
      id,
      header: label,
      enableSorting: false,
      cell: ({ row }) => (
        <span className={subtle ? 'text-fg-subtle' : undefined}>
          {price(row.original.pricing?.[id])}
        </span>
      ),
      meta: { label, width: 112, align: 'end', numeric: true },
    });

  const columns = useMemo(
    () =>
      column.columns([
        column.accessor('name', {
          id: 'name',
          header: t('table.name'),
          enableHiding: false,
          cell: ({ row }) => {
            const model = row.original;
            return (
              <div
                className={`flex min-w-0 flex-col gap-0.5 py-1.5 ${model.is_active ? '' : 'opacity-60'}`}
              >
                <span className="truncate font-mono text-mono text-fg">
                  {model.name}
                </span>
                {model.deployments > 1 && (
                  <span className="text-hint text-fg-subtle">
                    {model.weight === null
                      ? t('deployment')
                      : t('deploymentWeight', { weight: model.weight })}
                  </span>
                )}
              </div>
            );
          },
          meta: { label: t('table.name'), width: 220 },
        }),
        column.accessor('provider', {
          id: 'provider',
          header: t('table.provider'),
          cell: ({ getValue }) => (
            <Tag variant="soft">{providerLabel(getValue())}</Tag>
          ),
          meta: { label: t('table.provider'), width: 140 },
        }),
        column.accessor('upstream_model', {
          id: 'upstream_model',
          header: t('table.upstream'),
          cell: ({ row }) => (
            <div className="flex min-w-0 flex-col gap-0.5 py-1.5">
              <span className="truncate font-mono text-mono-sm text-fg-secondary">
                {row.original.upstream_model}
              </span>
              <span className="truncate font-mono text-mono-sm text-fg-subtle">
                {row.original.effective_api_base}
              </span>
            </div>
          ),
          meta: { label: t('table.upstream'), width: 240 },
        }),
        column.display({
          id: 'pricing',
          header: t('table.pricing'),
          enableSorting: false,
          cell: ({ row }) => <PricingSource model={row.original} />,
          meta: { label: t('table.pricing'), width: 160 },
        }),
        priceColumn('input', t('table.input')),
        priceColumn('output', t('table.output')),
        priceColumn('cache_read', t('table.cacheRead'), true),
        column.accessor('has_api_key', {
          id: 'key',
          header: t('table.key'),
          enableSorting: false,
          cell: ({ getValue }) =>
            getValue() ? (
              <Tag tone="brand">{t('keyStored')}</Tag>
            ) : (
              <span className="text-fg-subtle">{t('keyNone')}</span>
            ),
          meta: { label: t('table.key'), width: 112 },
        }),
        column.accessor('is_active', {
          id: 'active',
          header: t('table.active'),
          enableSorting: false,
          cell: ({ row }) => (
            // The switch toggles; it does not open the row.
            <span onClick={(event) => event.stopPropagation()}>
              <Switch
                checked={row.original.is_active}
                disabled={!isAdmin}
                onCheckedChange={(next) => onToggle(row.original, next)}
                aria-label={t('activeLabel', { name: row.original.name })}
              />
            </span>
          ),
          meta: { label: t('table.active'), width: 88 },
        }),
      ]),
    [t, money, providerLabel, isAdmin, onToggle],
  );

  const rowMenu = useMemo(
    () =>
      isAdmin
        ? ({ original: model }: { original: Model }): MenuAction[] => [
            {
              id: 'edit',
              label: t('edit'),
              icon: EditIcon,
              onSelect: () => onEdit(model),
            },
            {
              id: 'test',
              label: t('test.button'),
              icon: TestIcon,
              onSelect: () => onTest(model),
            },
            {
              id: 'delete',
              label: t('delete'),
              icon: DeleteIcon,
              tone: 'danger',
              separated: true,
              onSelect: () => onDelete(model),
            },
          ]
        : undefined,
    [isAdmin, t, onEdit, onTest, onDelete],
  );

  const table = useListTable({
    data: models,
    columns,
    total,
    params,
    defaultSort: MODELS_DEFAULT_SORT,
    onParams,
    getRowId: (model) => String(model.id),
    rowMenu,
    storageKey: 'models',
  });

  return (
    <DataTable
      table={table}
      className="min-h-0 flex-1"
      empty={empty}
      onRowClick={isAdmin ? (row) => onEdit(row.original) : undefined}
    />
  );
}
