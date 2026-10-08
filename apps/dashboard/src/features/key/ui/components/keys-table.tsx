import { Link, useNavigate } from '@tanstack/react-router';
import type React from 'react';
import { useTranslations } from 'use-intl';
import type { ApiKey } from '@/shared/api/schemas';
import {
  ActionsCell,
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
  SortHead,
} from '@/shared/ui/components/data-table';
import { LabelBadges } from '@/shared/ui/components/label-badge';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { SpendBar } from '@/shared/ui/components/spend-bar';
import { useMoney } from '@/shared/ui/hooks/use-money';

export function ModelBadges({
  models,
  allLabel,
}: {
  models: { id: number; name: string }[];
  allLabel: string;
}) {
  if (models.length === 0) {
    return <span className="text-sm text-muted-foreground">{allLabel}</span>;
  }
  return (
    <div className="flex max-w-64 flex-wrap gap-1">
      {models.map((model) => (
        <Badge key={model.id} variant="outline" className="font-mono">
          {model.name}
        </Badge>
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
      <Badge variant="secondary">{t('personal')}</Badge>
      <span className="text-muted-foreground">{apiKey.owner_email}</span>
    </span>
  );
}

/** Blocked, expired, or when it expires. */
export function KeyStatus({ apiKey }: { apiKey: ApiKey }) {
  const t = useTranslations('keys');
  const money = useMoney();
  if (apiKey.blocked) {
    return <Badge variant="destructive">{t('blocked')}</Badge>;
  }
  if (apiKey.expires_at === null) {
    return <span className="text-muted-foreground">{t('never')}</span>;
  }
  if (Date.parse(apiKey.expires_at) <= Date.now()) {
    return <Badge variant="destructive">{t('expired')}</Badge>;
  }
  return <>{money.date(apiKey.expires_at)}</>;
}

export function KeysTable({
  keys,
  actions,
  showTeam = true,
  sort,
  onSort,
  toolbar,
  footer,
  empty,
}: {
  keys: ApiKey[];
  actions: (key: ApiKey) => React.ReactNode;
  showTeam?: boolean;
  /** With `onSort`, headers sort the list (server field names). */
  sort?: string;
  onSort?: (sort: string | undefined) => void;
  toolbar?: React.ReactNode;
  footer?: React.ReactNode;
  /** Shown instead of rows when there are none. */
  empty: { title: string; description?: string; action?: React.ReactNode };
}) {
  const t = useTranslations('keys');
  const money = useMoney();
  const head = (field: string, label: string, className?: string) =>
    onSort ? (
      <SortHead
        field={field}
        sort={sort}
        fallback={KEYS_DEFAULT_SORT}
        onSort={onSort}
        className={className}
      >
        {label}
      </SortHead>
    ) : (
      <ColumnHead className={className}>{label}</ColumnHead>
    );
  return (
    <DataTable
      toolbar={toolbar}
      footer={footer}
      empty={keys.length === 0 && <DataTableEmpty {...empty} />}
      head={
        <>
          {head('name', t('table.name'), 'min-w-56')}
          {showTeam && head('team', t('table.owner'))}
          <ColumnHead>{t('table.models')}</ColumnHead>
          {head('spend', t('table.spend'), 'min-w-40')}
          {head('expires_at', t('table.expires'))}
          {head('last_used_at', t('table.lastUsed'))}
          <ColumnHead>{t('table.createdBy')}</ColumnHead>
          <ColumnHead className="w-12" />
        </>
      }
    >
      {keys.map((key) => (
        <KeyRow key={key.id} apiKey={key}>
          <Cell>
            <div className="flex min-w-0 flex-col gap-0.5">
              <Link
                to="/keys/$keyId"
                params={{ keyId: String(key.id) }}
                className="truncate font-medium underline-offset-4 hover:underline"
              >
                {key.name}
              </Link>
              <span className="font-mono text-xs text-muted-foreground">
                {key.key_hint}
                {key.person_name && (
                  <span className="font-sans"> · {key.person_name}</span>
                )}
              </span>
              <LabelBadges labels={key.labels} />
            </div>
          </Cell>
          {showTeam && (
            <Cell>
              <KeyOwner apiKey={key} />
            </Cell>
          )}
          <Cell>
            <ModelBadges models={key.models} allLabel={t('table.allModels')} />
          </Cell>
          <Cell>
            <SpendBar spent={key.spend_usd} budget={key.max_budget_usd} />
          </Cell>
          <Cell className="whitespace-nowrap">
            <KeyStatus apiKey={key} />
          </Cell>
          <Cell className="whitespace-nowrap text-muted-foreground">
            {key.last_used_at ? money.dateTime(key.last_used_at) : t('never')}
          </Cell>
          <Cell className="text-muted-foreground">
            {key.created_by_email ?? '—'}
          </Cell>
          <ActionsCell>{actions(key)}</ActionsCell>
        </KeyRow>
      ))}
    </DataTable>
  );
}

/** What `GET /api/keys` sorts by when not told. */
export const KEYS_DEFAULT_SORT = '-created_at';

function KeyRow({
  apiKey,
  children,
}: {
  apiKey: ApiKey;
  children: React.ReactNode;
}) {
  const navigate = useNavigate();
  return (
    <DataRow
      onOpen={() =>
        navigate({ to: '/keys/$keyId', params: { keyId: String(apiKey.id) } })
      }
    >
      {children}
    </DataRow>
  );
}
