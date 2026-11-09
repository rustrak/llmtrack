import {
  Avatar,
  Button,
  Count,
  createDataTableColumnHelper,
  DataTable,
  DeleteIcon,
  MailIcon,
  Menu,
  MoreIcon,
  NewIcon,
  Switch,
  Tab,
  TabList,
  TabPanel,
  Tabs,
} from '@rustrak/ui';
import {
  createFileRoute,
  redirect,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { deleteUser, updateUser } from '@/features/user/api/mutations';
import {
  listInvitations,
  listUsers,
  type UserListParams,
} from '@/features/user/api/queries';
import { CreateUserDialog } from '@/features/user/ui/components/create-user-dialog';
import { InviteDialog } from '@/features/user/ui/components/invite-dialog';
import { PendingInvitations } from '@/features/user/ui/components/pending-invitations';
import { unwrap } from '@/shared/api/http';
import type { UserListItem } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { parseListParams } from '@/shared/lib/list-params';
import { Choice } from '@/shared/ui/components/choice';
import { ConfirmDialog } from '@/shared/ui/components/confirm-dialog';
import { FilterPill, ListQueryBar } from '@/shared/ui/components/list-filters';
import { Page } from '@/shared/ui/components/page';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useListTable } from '@/shared/ui/hooks/use-list-table';
import { useMoney } from '@/shared/ui/hooks/use-money';

const ROLES = ['admin', 'member'] as const;
const STATUSES = ['active', 'inactive'] as const;
/** What `GET /api/users` sorts by when not told. */
const DEFAULT_SORT = 'email';
const column = createDataTableColumnHelper<UserListItem>();

export const Route = createFileRoute('/_authenticated/users')({
  beforeLoad: ({ context }) => {
    if (context.state !== 'authenticated' || context.user.role !== 'admin') {
      throw redirect({ to: '/keys' });
    }
  },
  validateSearch: (search: Record<string, unknown>): UserListParams => {
    const role = ROLES.find((r) => r === search.role);
    const status = STATUSES.find((s) => s === search.status);
    return {
      ...parseListParams(search),
      ...(role ? { role } : {}),
      ...(status ? { status } : {}),
    };
  },
  loaderDeps: ({ search }) => search,
  loader: async ({ deps }) => {
    const [users, invitations] = await Promise.all([
      listUsers(deps),
      listInvitations(),
    ]);
    return { users: unwrap(users), invitations: unwrap(invitations) };
  },
  head: () => ({ meta: [{ title: translator('users')('meta.title') }] }),
  component: UsersPage,
});

function UsersPage() {
  const t = useTranslations('users');
  const tRoles = useTranslations('roles');
  const { users, invitations } = Route.useLoaderData();
  const me = useCurrentUser();
  const router = useRouter();
  const money = useMoney();
  const [adding, setAdding] = useState<'invite' | 'create' | null>(null);
  const [deleting, setDeleting] = useState<UserListItem | null>(null);
  const tTable = useTranslations('table');
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const set = (patch: UserListParams) =>
    navigate({ search: (prev) => ({ ...prev, page: undefined, ...patch }) });
  const filtered = Boolean(search.q || search.role || search.status);
  const pending = invitations.filter((i) => i.status === 'pending').length;
  const roleOptions = ROLES.map((value) => ({ value, label: tRoles(value) }));
  const statusOptions = STATUSES.map((value) => ({
    value,
    label: t(`statusFilter.${value}`),
  }));

  const columns = useMemo(() => {
    const update = async (
      user: UserListItem,
      patch: { role?: string; is_active?: boolean },
    ) => {
      const result = await updateUser(user.id, patch);
      if (!result.success) toast.error(readableMessage(result.error));
      await router.invalidate();
    };
    return column.columns([
      column.accessor('email', {
        id: 'email',
        header: t('table.user'),
        enableHiding: false,
        cell: ({ row }) => {
          const user = row.original;
          return (
            <div className="flex min-w-0 items-center gap-3 py-1.5">
              <Avatar name={user.name || user.email} />
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="truncate text-value font-medium text-fg">
                  {user.name || user.email}
                  {user.id === me.id && (
                    <span className="font-normal text-fg-subtle">
                      {' '}
                      {t('you')}
                    </span>
                  )}
                </span>
                {user.name && (
                  <span className="truncate text-hint text-fg-subtle">
                    {user.email}
                  </span>
                )}
              </div>
            </div>
          );
        },
        meta: { label: t('table.user') },
      }),
      column.accessor('role', {
        id: 'role',
        header: t('table.role'),
        cell: ({ row }) => {
          const user = row.original;
          return (
            <Choice
              aria-label={t('roleOf', { email: user.email })}
              size="sm"
              className="w-32"
              disabled={user.id === me.id}
              value={user.role}
              onChange={(role) => update(user, { role })}
              options={ROLES.map((value) => ({
                value,
                label: tRoles(value),
              }))}
            />
          );
        },
        meta: { label: t('table.role'), width: 152 },
      }),
      column.accessor('spend_usd', {
        id: 'spend',
        header: t('table.spend'),
        cell: ({ getValue }) => money.usd(getValue()),
        meta: { label: t('table.spend'), width: 128, numeric: true },
      }),
      column.accessor('key_count', {
        id: 'keys',
        header: t('table.keys'),
        meta: { label: t('table.keys'), width: 96, numeric: true },
      }),
      column.accessor('is_active', {
        id: 'is_active',
        header: t('table.active'),
        enableSorting: false,
        cell: ({ row }) => {
          const user = row.original;
          return (
            <Switch
              checked={user.is_active}
              disabled={user.id === me.id}
              onCheckedChange={(is_active) => update(user, { is_active })}
              aria-label={t('activeOf', { email: user.email })}
            />
          );
        },
        meta: { label: t('table.active'), width: 96 },
      }),
      column.accessor('last_login', {
        id: 'last_login',
        header: t('table.lastLogin'),
        cell: ({ getValue }) => {
          const at = getValue();
          return (
            <span className="whitespace-nowrap text-fg-subtle tabular-nums">
              {at ? money.dateTime(at) : t('never')}
            </span>
          );
        },
        meta: { label: t('table.lastLogin'), width: 156 },
      }),
      column.accessor('created_at', {
        id: 'created_at',
        header: t('table.created'),
        cell: ({ getValue }) => (
          <span className="whitespace-nowrap text-fg-subtle tabular-nums">
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
          const user = row.original;
          if (user.id === me.id) return null;
          return (
            <Menu
              align="end"
              trigger={
                <Button
                  variant="ghost"
                  size="sm"
                  icon={MoreIcon}
                  aria-label={t('deleteOf', { email: user.email })}
                />
              }
              actions={[
                {
                  id: 'delete',
                  label: t('deleteDialog.confirm'),
                  icon: DeleteIcon,
                  tone: 'danger',
                  onSelect: () => setDeleting(user),
                },
              ]}
            />
          );
        },
        meta: { width: 56, align: 'end' },
      }),
    ]);
  }, [t, tRoles, money, me.id, router]);

  const table = useListTable({
    data: users.data,
    columns,
    total: users.total,
    params: search,
    defaultSort: DEFAULT_SORT,
    onParams: (patch) =>
      navigate({ search: (prev) => ({ ...prev, ...patch }) }),
    getRowId: (user) => String(user.id),
    storageKey: 'users',
  });

  return (
    <Tabs defaultValue="users" className="flex h-full min-h-0 flex-col">
      <Page
        fill
        title={t('title')}
        description={t('subtitle')}
        actions={
          <>
            <Button icon={NewIcon} onClick={() => setAdding('create')}>
              {t('add')}
            </Button>
            <Button
              variant="primary"
              icon={MailIcon}
              onClick={() => setAdding('invite')}
            >
              {t('invite.button')}
            </Button>
          </>
        }
        tabs={
          <TabList>
            <Tab value="users">{t('tabs.users')}</Tab>
            <Tab value="invitations">
              {t('tabs.invitations')}
              {pending > 0 && <Count>{pending}</Count>}
            </Tab>
          </TabList>
        }
      >
        <TabPanel value="users" className="flex min-h-0 flex-1 flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <ListQueryBar
              className="min-w-0 flex-1 sm:max-w-md max-sm:basis-full"
              placeholder={t('search')}
              search={search}
              onChange={(patch) => set(patch as UserListParams)}
              fields={[
                {
                  key: 'role',
                  label: t('filters.role'),
                  variant: 'options',
                  multiple: false,
                  options: roleOptions,
                },
                {
                  key: 'status',
                  label: t('filters.status'),
                  variant: 'options',
                  multiple: false,
                  options: statusOptions,
                },
              ]}
            />
            <FilterPill
              label={t('filters.role')}
              value={search.role}
              onChange={(v) => set({ role: ROLES.find((r) => r === v) })}
              options={roleOptions}
            />
            <FilterPill
              label={t('filters.status')}
              value={search.status}
              onChange={(v) => set({ status: STATUSES.find((s) => s === v) })}
              options={statusOptions}
            />
            {filtered && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  set({ q: undefined, role: undefined, status: undefined })
                }
              >
                {tTable('clearFilters')}
              </Button>
            )}
          </div>
          <DataTable
            table={table}
            className="min-h-0 flex-1"
            empty={{
              title: tTable('noResults'),
              description: tTable('noResultsHint'),
            }}
          />
        </TabPanel>
        <TabPanel value="invitations" className="min-h-0 overflow-y-auto">
          <PendingInvitations invitations={invitations} />
        </TabPanel>
        {adding === 'create' && (
          <CreateUserDialog
            onClose={() => setAdding(null)}
            onSaved={() => router.invalidate()}
          />
        )}
        {adding === 'invite' && (
          <InviteDialog
            onClose={() => setAdding(null)}
            onSaved={() => router.invalidate()}
          />
        )}
        <ConfirmDialog
          open={deleting !== null}
          onOpenChange={(open) => !open && setDeleting(null)}
          title={t('deleteDialog.title', { email: deleting?.email ?? '' })}
          description={t('deleteDialog.description')}
          confirmLabel={t('deleteDialog.confirm')}
          onConfirm={async () => {
            if (!deleting) return null;
            const result = await deleteUser(deleting.id);
            if (!result.success) return readableMessage(result.error);
            await router.invalidate();
            return null;
          }}
        />
      </Page>
    </Tabs>
  );
}
