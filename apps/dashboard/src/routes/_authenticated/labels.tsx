import {
  Button,
  createDataTableColumnHelper,
  DataTable,
  DeleteIcon,
  EditIcon,
  NewIcon,
} from '@rustrak/ui';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
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
import { LabelBadge } from '@/shared/ui/components/label-badge';
import { ListQueryBar } from '@/shared/ui/components/list-filters';
import { Page } from '@/shared/ui/components/page';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** What `GET /api/labels` sorts by when not told. */
const DEFAULT_SORT = 'name';
const column = createDataTableColumnHelper<Label>();

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

  const columns = useMemo(
    () =>
      column.columns([
        column.accessor('name', {
          id: 'name',
          header: t('table.name'),
          enableHiding: false,
          cell: ({ row }) => <LabelBadge label={row.original} />,
          meta: { label: t('table.name'), width: 220 },
        }),
        column.accessor('description', {
          id: 'description',
          header: t('table.description'),
          enableSorting: false,
          cell: ({ getValue }) => (
            <span className="truncate text-fg-subtle">{getValue() ?? '—'}</span>
          ),
          meta: { label: t('table.description') },
        }),
        column.accessor('key_count', {
          id: 'keys',
          header: t('table.keys'),
          cell: ({ row }) => {
            const label = row.original;
            return (
              <span className="font-mono text-mono-sm tabular-nums">
                {label.key_count > 0 ? (
                  <Link
                    to="/usage"
                    search={{ label: label.id }}
                    className="text-fg-brand hover:underline"
                    title={t('table.usage')}
                  >
                    {label.key_count}
                  </Link>
                ) : (
                  0
                )}
              </span>
            );
          },
          meta: {
            label: t('table.keys'),
            width: 96,
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
      ]),
    [t, money],
  );

  const table = useListTable({
    data: labels.data,
    columns,
    total: labels.total,
    params: search,
    defaultSort: DEFAULT_SORT,
    onParams: (patch) =>
      navigate({ search: (prev) => ({ ...prev, ...patch }) }),
    getRowId: (label) => String(label.id),
    storageKey: 'labels',
    // Only admins edit or delete labels.
    rowMenu: isAdmin
      ? ({ original: label }) => [
          {
            id: 'edit',
            label: t('edit.label', { name: label.name }),
            icon: EditIcon,
            onSelect: () => setEditing({ label }),
          },
          {
            id: 'delete',
            label: t('delete.label', { name: label.name }),
            icon: DeleteIcon,
            tone: 'danger',
            onSelect: () => setDeleting(label),
          },
        ]
      : undefined,
  });

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
            onClick={() => setEditing({})}
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
            onChange={(patch) => set(patch as ListParams)}
            fields={[]}
          />
        </div>
        <DataTable
          table={table}
          className="min-h-0 flex-1"
          empty={
            search.q
              ? {
                  title: tTable('noResults'),
                  description: tTable('noResultsHint'),
                }
              : {
                  title: t('empty.title'),
                  description: isAdmin
                    ? t('empty.description')
                    : t('empty.member'),
                }
          }
        />
      </div>
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
