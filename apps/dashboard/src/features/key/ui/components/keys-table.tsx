import { createDataTableColumnHelper, DataTable, Tag } from '@rustrak/ui';
import { useNavigate } from '@tanstack/react-router';
import type React from 'react';
import { useMemo } from 'react';
import { useTranslations } from 'use-intl';
import type { ApiKey } from '@/shared/api/schemas';
import type { ListParams } from '@/shared/lib/list-params';
import { LabelBadges } from '@/shared/ui/components/label-badge';
import { SpendBar } from '@/shared/ui/components/spend-bar';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';

export function ModelBadges({
  models,
  allLabel,
}: {
  models: { id: number; name: string }[];
  allLabel: string;
}) {
  if (models.length === 0) {
    return <span className="text-fg-subtle">{allLabel}</span>;
  }
  return (
    <div className="flex max-w-64 flex-wrap gap-1">
      {models.map((model) => (
        <span
          key={model.id}
          className="inline-flex h-5 items-center rounded-sm bg-surface-chip px-1.5 font-mono text-mono-sm text-fg-secondary"
        >
          {model.name}
        </span>
      ))}
    </div>
  );
}

/** A team's name, or who a personal key belongs to. */
export function KeyOwner({ apiKey }: { apiKey: ApiKey }) {
  const t = useTranslations('keys');
  if (apiKey.team_name !== null) return <>{apiKey.team_name}</>;
  return (
    <span className="flex flex-wrap items-center gap-1.5">
      <Tag variant="soft">{t('personal')}</Tag>
      <span className="text-fg-subtle">{apiKey.owner_email}</span>
    </span>
  );
}

/** Blocked, expired, or when it expires. */
export function KeyStatus({ apiKey }: { apiKey: ApiKey }) {
  const t = useTranslations('keys');
  const money = useMoney();
  if (apiKey.blocked) {
    return <Tag tone="error">{t('blocked')}</Tag>;
  }
  if (apiKey.expires_at === null) {
    return <span className="text-fg-subtle">{t('never')}</span>;
  }
  if (Date.parse(apiKey.expires_at) <= Date.now()) {
    return <Tag tone="error">{t('expired')}</Tag>;
  }
  return <>{money.date(apiKey.expires_at)}</>;
}

const column = createDataTableColumnHelper<ApiKey>();

/**
 * The keys as the design system's table: server-paged and server-sorted, the
 * column ids being the API's sort names. A row opens the key's page; its last
 * cell is the key's own actions (a menu that also owns panels and dialogs).
 */
export function KeysTable({
  keys,
  total,
  params,
  onParams,
  actions,
  showTeam = true,
  empty,
}: {
  keys: ApiKey[];
  total: number;
  /** The list's URL params; their sort and page drive the table. */
  params: ListParams;
  onParams: (patch: Partial<ListParams>) => void;
  actions: (key: ApiKey) => React.ReactNode;
  showTeam?: boolean;
  empty: { title: string; description?: string };
}) {
  const t = useTranslations('keys');
  const money = useMoney();
  const navigate = useNavigate();

  const columns = useMemo(
    () =>
      column.columns([
        column.accessor('name', {
          id: 'name',
          header: t('table.name'),
          enableHiding: false,
          cell: ({ row }) => {
            const key = row.original;
            return (
              <div className="flex min-w-0 flex-col gap-0.5 py-1.5">
                <span className="truncate text-value font-medium text-fg">
                  {key.name}
                </span>
                <span className="truncate font-mono text-mono-sm text-fg-subtle">
                  {key.key_hint}
                  {key.person_name && (
                    <span className="font-sans text-meta">
                      {' '}
                      · {key.person_name}
                    </span>
                  )}
                </span>
                <LabelBadges labels={key.labels} />
              </div>
            );
          },
          meta: { label: t('table.name') },
        }),
        ...(showTeam
          ? [
              column.accessor('team_name', {
                id: 'team',
                header: t('table.owner'),
                cell: ({ row }) => <KeyOwner apiKey={row.original} />,
                meta: { label: t('table.owner'), width: 180 },
              }),
            ]
          : []),
        column.accessor('models', {
          id: 'models',
          header: t('table.models'),
          enableSorting: false,
          cell: ({ getValue }) => (
            <ModelBadges models={getValue()} allLabel={t('table.allModels')} />
          ),
          meta: { label: t('table.models'), width: 180 },
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
        column.accessor('expires_at', {
          id: 'expires_at',
          header: t('table.expires'),
          cell: ({ row }) => <KeyStatus apiKey={row.original} />,
          meta: { label: t('table.expires'), width: 128 },
        }),
        column.accessor('last_used_at', {
          id: 'last_used_at',
          header: t('table.lastUsed'),
          cell: ({ getValue }) => {
            const at = getValue();
            return (
              <span className="text-fg-subtle tabular-nums">
                {at ? money.dateTime(at) : t('never')}
              </span>
            );
          },
          meta: { label: t('table.lastUsed'), width: 156 },
        }),
        column.accessor('created_by_email', {
          id: 'created_by',
          header: t('table.createdBy'),
          enableSorting: false,
          cell: ({ getValue }) => (
            <span className="truncate text-fg-subtle">{getValue() ?? '—'}</span>
          ),
          meta: { label: t('table.createdBy'), width: 180 },
        }),
        column.display({
          id: 'actions',
          header: '',
          enableSorting: false,
          enableHiding: false,
          cell: ({ row }) => actions(row.original),
          meta: { width: 56, align: 'end' },
        }),
      ]),
    [t, money, showTeam, actions],
  );

  const table = useListTable({
    data: keys,
    columns,
    total,
    params,
    defaultSort: KEYS_DEFAULT_SORT,
    onParams,
    getRowId: (key) => String(key.id),
    storageKey: showTeam ? 'keys' : 'team-keys',
    initialVisibility: { created_by: false },
  });

  return (
    <DataTable
      table={table}
      className="min-h-0 flex-1"
      empty={empty}
      onRowClick={(row) =>
        navigate({
          to: '/keys/$keyId',
          params: { keyId: String(row.original.id) },
        })
      }
    />
  );
}

/** What `GET /api/keys` sorts by when not told. */
export const KEYS_DEFAULT_SORT = '-created_at';
