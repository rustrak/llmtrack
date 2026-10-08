import {
  createFileRoute,
  useNavigate,
  useRouter,
} from '@tanstack/react-router';
import { MoreHorizontal, Pencil, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { listKeys } from '@/features/key/api/queries';
import {
  CreateKeyButton,
  KeyManager,
} from '@/features/key/ui/components/key-manager';
import { listLabels } from '@/features/label/api/queries';
import { listModels } from '@/features/model/api/queries';
import { getSettings } from '@/features/settings/api/queries';
import { deleteTeam } from '@/features/team/api/mutations';
import { getTeam, listPeople } from '@/features/team/api/queries';
import { MembersPanel } from '@/features/team/ui/components/members-panel';
import { PeoplePanel } from '@/features/team/ui/components/people-panel';
import {
  DetailsCard,
  SpendCard,
  TeamFacts,
  TeamTitle,
} from '@/features/team/ui/components/team-overview';
import { TeamPanel } from '@/features/team/ui/components/team-panel';
import { getUsage } from '@/features/usage/api/queries';
import { rangeFor } from '@/features/usage/model/range';
import {
  StackedBars,
  useBucketLabel,
} from '@/features/usage/ui/components/usage-charts';
import { listUsers } from '@/features/user/api/queries';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { MAX_PAGE_SIZE } from '@/shared/lib/list-params';
import { ConfirmDialog } from '@/shared/ui/components/confirm-dialog';
import { Page } from '@/shared/ui/components/page';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/shared/ui/components/shadcn/dropdown-menu';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { useMoney } from '@/shared/ui/hooks/use-money';

export const Route = createFileRoute('/_authenticated/teams/$teamId')({
  loader: async ({ params, context }) => {
    const id = Number(params.teamId);
    const isAdmin =
      context.state === 'authenticated' && context.user.role === 'admin';
    const [team, keys, models, labels, settings, unassigned, users, usage] =
      await Promise.all([
        getTeam(id),
        // ponytail: a team's first 200 keys; page it like /keys if teams get bigger.
        listKeys({ team_id: id, per_page: MAX_PAGE_SIZE }),
        listModels({ per_page: MAX_PAGE_SIZE }),
        listLabels({ per_page: MAX_PAGE_SIZE }),
        getSettings(),
        // People without a team and the user list are for global admins only.
        isAdmin
          ? listPeople({ unassigned: true, per_page: MAX_PAGE_SIZE })
          : null,
        isAdmin ? listUsers({ per_page: MAX_PAGE_SIZE }) : null,
        getUsage({ ...rangeFor('30d'), team_id: id }),
      ]);
    return {
      unassigned: unassigned ? unwrap(unassigned).data : [],
      users: users ? unwrap(users).data : undefined,
      team: unwrap(team),
      keys: unwrap(keys).data,
      models: unwrap(models).data,
      labels: unwrap(labels).data,
      publicUrl: unwrap(settings).public_url,
      usage: unwrap(usage),
    };
  },
  head: ({ loaderData }) => ({
    meta: [
      {
        title: translator('teams')('meta.detailTitle', {
          name: loaderData?.team.name ?? '',
        }),
      },
    ],
  }),
  component: TeamPage,
});

function TeamPage() {
  const t = useTranslations('teams');
  const tKeys = useTranslations('keys');
  const tUsage = useTranslations('usage');
  const { team, keys, models, labels, publicUrl, unassigned, users, usage } =
    Route.useLoaderData();
  const user = useCurrentUser();
  const router = useRouter();
  const navigate = useNavigate();
  const money = useMoney();
  const bucketLabel = useBucketLabel('day');
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const isAdmin = user.role === 'admin';
  const canManageMembers = isAdmin || team.my_role === 'admin';
  const modelRefs = models.map(({ id, name }) => ({ id, name }));
  const count = (n: number) => (
    <Badge variant="secondary" className="ml-1.5 tabular-nums">
      {n}
    </Badge>
  );

  return (
    <Page
      title={<TeamTitle name={team.name} />}
      description={<TeamFacts team={team} />}
      actions={
        isAdmin && (
          <>
            <Button variant="outline" onClick={() => setEditing(true)}>
              <Pencil />
              {t('detail.edit')}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    variant="outline"
                    size="icon"
                    aria-label={t('detail.moreActions')}
                  />
                }
              >
                <MoreHorizontal />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  className="text-destructive"
                  onClick={() => setDeleting(true)}
                >
                  <Trash2 />
                  {t('detail.deleteTeam')}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </>
        )
      }
    >
      <div className="grid gap-4 lg:grid-cols-3">
        <SpendCard
          team={team}
          usage={usage}
          chart={
            <StackedBars
              height="h-44"
              rows={usage.daily.map((d) => ({
                bucket: d.day,
                spend: d.cost_usd,
              }))}
              series={[
                {
                  key: 'spend',
                  label: tUsage('spend'),
                  color: 'var(--chart-1)',
                },
              ]}
              format={(v) => money.usd(v)}
              bucketLabel={bucketLabel}
            />
          }
        />
        <DetailsCard team={team} />
      </div>
      <Tabs defaultValue="keys">
        <TabsList>
          <TabsTrigger value="keys">
            {t('detail.keysTab')}
            {count(keys.length)}
          </TabsTrigger>
          <TabsTrigger value="people">
            {t('detail.peopleTab')}
            {count(team.people.length)}
          </TabsTrigger>
          <TabsTrigger value="members">
            {t('detail.membersTab')}
            {count(team.members.length)}
          </TabsTrigger>
        </TabsList>
        <TabsContent value="keys" className="pt-4">
          <KeyManager
            keys={keys}
            teams={[team]}
            allModels={modelRefs}
            labels={labels}
            teamId={team.id}
            publicUrl={publicUrl}
            toolbar={
              <div className="ml-auto">
                <CreateKeyButton
                  teams={[team]}
                  allModels={modelRefs}
                  labels={labels}
                  teamId={team.id}
                  publicUrl={publicUrl}
                />
              </div>
            }
            empty={{
              title: tKeys('empty.title'),
              description: tKeys('empty.description'),
            }}
          />
        </TabsContent>
        <TabsContent value="people" className="pt-4">
          <PeoplePanel
            teamId={team.id}
            people={team.people}
            unassigned={unassigned}
            canManage={canManageMembers}
          />
        </TabsContent>
        <TabsContent value="members" className="pt-4">
          <MembersPanel
            teamId={team.id}
            members={team.members}
            candidates={users}
            canManage={canManageMembers}
          />
        </TabsContent>
      </Tabs>
      <TeamPanel
        open={editing}
        team={team}
        models={modelRefs}
        onClose={() => setEditing(false)}
        onSaved={() => router.invalidate()}
      />
      <ConfirmDialog
        open={deleting}
        onOpenChange={setDeleting}
        title={t('deleteDialog.title', { name: team.name })}
        description={t('deleteDialog.description')}
        confirmLabel={t('deleteDialog.confirm')}
        onConfirm={async () => {
          const result = await deleteTeam(team.id);
          if (!result.success) return readableMessage(result.error);
          await navigate({ to: '/teams' });
          return null;
        }}
      />
    </Page>
  );
}
