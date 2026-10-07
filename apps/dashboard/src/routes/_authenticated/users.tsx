import {
  createFileRoute,
  redirect,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';
import { Mail, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
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
import { Switch } from '@/shared/ui/components/shadcn/switch';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useMoney } from '@/shared/ui/hooks/use-money';

const ROLES = ['admin', 'member'] as const;
const STATUSES = ['active', 'inactive'] as const;
/** What `GET /api/users` sorts by when not told. */
const DEFAULT_SORT = 'email';

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

  const update = async (
    user: UserListItem,
    patch: { role?: string; is_active?: boolean },
  ) => {
    const result = await updateUser(user.id, patch);
    if (!result.success) toast.error(readableMessage(result.error));
    await router.invalidate();
  };

  return (
    <Page
      fill
      title={t('title')}
      description={t('subtitle')}
      actions={
        <>
          <Button variant="outline" onClick={() => setAdding('create')}>
            <Plus />
            {t('add')}
          </Button>
          <Button onClick={() => setAdding('invite')}>
            <Mail />
            {t('invite.button')}
          </Button>
        </>
      }
    >
      <Tabs defaultValue="users" className="min-h-0 flex-1">
        <TabsList>
          <TabsTrigger value="users">{t('tabs.users')}</TabsTrigger>
          <TabsTrigger value="invitations">
            {t('tabs.invitations')}
            {pending > 0 && (
              <Badge variant="secondary" className="tabular-nums">
                {pending}
              </Badge>
            )}
          </TabsTrigger>
        </TabsList>
        <TabsContent value="users" className="flex min-h-0 flex-col pt-2">
          <DataTable
            toolbar={
              <>
                <SearchField
                  value={search.q}
                  onChange={(q) => set({ q })}
                  placeholder={t('search')}
                />
                <FilterPill
                  label={t('filters.role')}
                  value={search.role}
                  onChange={(v) => set({ role: ROLES.find((r) => r === v) })}
                  options={ROLES.map((value) => ({
                    value,
                    label: tRoles(value),
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
                      set({ q: undefined, role: undefined, status: undefined })
                    }
                  >
                    {tTable('clearFilters')}
                  </Button>
                )}
              </>
            }
            head={
              <>
                {sortable('email', t('table.user'))}
                {sortable('role', t('table.role'))}
                {sortable('spend', t('table.spend'))}
                {sortable('keys', t('table.keys'))}
                <ColumnHead>{t('table.active')}</ColumnHead>
                {sortable('last_login', t('table.lastLogin'))}
                {sortable('created_at', t('table.created'))}
                <ColumnHead className="w-12" />
              </>
            }
            empty={
              users.data.length === 0 && (
                <DataTableEmpty
                  title={tTable('noResults')}
                  description={tTable('noResultsHint')}
                />
              )
            }
            footer={
              <ListFooter
                list={users}
                onChange={(patch) =>
                  navigate({ search: (prev) => ({ ...prev, ...patch }) })
                }
              />
            }
          >
            {users.data.map((user) => {
              const self = user.id === me.id;
              return (
                <DataRow key={user.id}>
                  <Cell>
                    <p className="font-medium">
                      {user.name || user.email}
                      {self && (
                        <span className="ml-2 text-xs font-normal text-muted-foreground">
                          {t('you')}
                        </span>
                      )}
                    </p>
                    {user.name && (
                      <p className="text-xs text-muted-foreground">
                        {user.email}
                      </p>
                    )}
                  </Cell>
                  <Cell>
                    <Choice
                      aria-label={t('roleOf', { email: user.email })}
                      className="h-8 w-32"
                      disabled={self}
                      value={user.role}
                      onChange={(role) => update(user, { role })}
                      options={ROLES.map((value) => ({
                        value,
                        label: tRoles(value),
                      }))}
                    />
                  </Cell>
                  <Cell className="tabular-nums">
                    {money.usd(user.spend_usd)}
                  </Cell>
                  <Cell className="tabular-nums">{user.key_count}</Cell>
                  <Cell>
                    <Switch
                      checked={user.is_active}
                      disabled={self}
                      onCheckedChange={(is_active) =>
                        update(user, { is_active })
                      }
                      aria-label={t('activeOf', { email: user.email })}
                    />
                  </Cell>
                  <Cell className="whitespace-nowrap text-muted-foreground">
                    {user.last_login
                      ? money.dateTime(user.last_login)
                      : t('never')}
                  </Cell>
                  <Cell className="whitespace-nowrap text-muted-foreground">
                    {money.date(user.created_at)}
                  </Cell>
                  <ActionsCell>
                    {!self && (
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={t('deleteOf', { email: user.email })}
                        onClick={() => setDeleting(user)}
                      >
                        <Trash2 />
                      </Button>
                    )}
                  </ActionsCell>
                </DataRow>
              );
            })}
          </DataTable>
        </TabsContent>
        <TabsContent
          value="invitations"
          className="min-h-0 overflow-y-auto pt-2"
        >
          <PendingInvitations invitations={invitations} />
        </TabsContent>
      </Tabs>
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
  );
}
