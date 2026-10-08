import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { deleteLabel } from '@/features/label/api/mutations';
import { listLabels } from '@/features/label/api/queries';
import { LabelPanel } from '@/features/label/ui/components/label-panel';
import { unwrap } from '@/shared/api/http';
import type { Label } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { type ListParams, parseListParams } from '@/shared/lib/list-params';
import { ConfirmDialog } from '@/shared/ui/components/confirm-dialog';
import {
  ActionsCell,
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
  ListFooter,
  SearchField,
  SortHead,
} from '@/shared/ui/components/data-table';
import { LabelBadge } from '@/shared/ui/components/label-badge';
import { Page } from '@/shared/ui/components/page';
import { Button } from '@/shared/ui/components/shadcn/button';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** What `GET /api/labels` sorts by when not told. */
const DEFAULT_SORT = 'name';

export const Route = createFileRoute('/_authenticated/labels')({
  validateSearch: (search: Record<string, unknown>): ListParams =>
    parseListParams(search),
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => ({ labels: unwrap(await listLabels(deps)) }),
  head: () => ({ meta: [{ title: translator('labels')('meta.title') }] }),
  component: LabelsPage,
});

function LabelsPage() {
  const t = useTranslations('labels');
  const tTable = useTranslations('table');
  const { labels } = Route.useLoaderData();
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const money = useMoney();
  const isAdmin = useCurrentUser().role === 'admin';
  // `null`: closed; `undefined` inside: a new label.
  const [editing, setEditing] = useState<{ label?: Label } | null>(null);
  const [deleting, setDeleting] = useState<Label | null>(null);
  const set = (patch: ListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const reload = () => navigate({ search: (prev) => prev });
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
          <Button onClick={() => setEditing({})}>
            <Plus />
            {t('create.button')}
          </Button>
        )
      }
    >
      <DataTable
        toolbar={
          <SearchField
            value={search.q}
            onChange={(q) => set({ q })}
            placeholder={t('search')}
          />
        }
        head={
          <>
            {sortable('name', t('table.name'), 'min-w-48')}
            <ColumnHead className="min-w-64">
              {t('table.description')}
            </ColumnHead>
            {sortable('keys', t('table.keys'))}
            {sortable('created_at', t('table.created'))}
            <ColumnHead className="w-20" />
          </>
        }
        empty={
          labels.data.length === 0 &&
          (search.q ? (
            <DataTableEmpty
              title={tTable('noResults')}
              description={tTable('noResultsHint')}
            />
          ) : (
            <DataTableEmpty
              title={t('empty.title')}
              description={isAdmin ? t('empty.description') : t('empty.member')}
            />
          ))
        }
        footer={
          <ListFooter
            list={labels}
            onChange={(patch) =>
              navigate({ search: (prev) => ({ ...prev, ...patch }) })
            }
          />
        }
      >
        {labels.data.map((label) => (
          <DataRow key={label.id}>
            <Cell>
              <LabelBadge label={label} />
            </Cell>
            <Cell className="text-muted-foreground">
              {label.description ?? '—'}
            </Cell>
            <Cell className="tabular-nums">
              {label.key_count > 0 ? (
                <Link
                  to="/usage"
                  search={{ label: label.id }}
                  className="underline-offset-4 hover:underline"
                  title={t('table.usage')}
                >
                  {label.key_count}
                </Link>
              ) : (
                0
              )}
            </Cell>
            <Cell className="whitespace-nowrap text-muted-foreground">
              {money.date(label.created_at)}
            </Cell>
            <ActionsCell>
              {isAdmin && (
                <div className="flex justify-end">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t('edit.label', { name: label.name })}
                    title={t('edit.label', { name: label.name })}
                    onClick={() => setEditing({ label })}
                  >
                    <Pencil />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t('delete.label', { name: label.name })}
                    title={t('delete.label', { name: label.name })}
                    onClick={() => setDeleting(label)}
                  >
                    <Trash2 />
                  </Button>
                </div>
              )}
            </ActionsCell>
          </DataRow>
        ))}
      </DataTable>
      <LabelPanel
        open={editing !== null}
        label={editing?.label}
        onClose={() => setEditing(null)}
        onSaved={reload}
      />
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title={t('delete.title', { name: deleting?.name ?? '' })}
        description={t('delete.description', {
          count: deleting?.key_count ?? 0,
        })}
        confirmLabel={t('delete.confirm')}
        onConfirm={async () => {
          if (!deleting) return null;
          const result = await deleteLabel(deleting.id);
          if (!result.success) return readableMessage(result.error);
          await reload();
          return null;
        }}
      />
    </Page>
  );
}
