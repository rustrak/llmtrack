import { useRouter } from '@tanstack/react-router';
import { Trash2 } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Member } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
import {
  ActionsCell,
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
} from '@/shared/ui/components/data-table';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';
import { addMember, removeMember, updateMember } from '../../api/mutations';

/**
 * Who may open the team in the dashboard, and as what. Access only: spend
 * is the People tab's.
 */
export function MembersPanel({
  teamId,
  members,
  candidates,
  canManage,
}: {
  teamId: number;
  members: Member[];
  /**
   * Users to pick from; only global admins can list them. Without it the
   * user is named by email.
   */
  candidates?: { email: string }[];
  canManage: boolean;
}) {
  const t = useTranslations('teams');
  const router = useRouter();
  const roles = [
    { value: 'member', label: t('members.roleMember') },
    { value: 'admin', label: t('members.roleAdmin') },
  ];
  const [email, setEmail] = useState('');
  const [role, setRole] = useState('member');
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const run = async (
    action: () => Promise<{ success: boolean; error?: unknown }>,
    done?: string,
  ) => {
    const result = await action();
    if (!result.success) {
      toast.error(
        readableMessage(result.error as Parameters<typeof readableMessage>[0]),
      );
      return false;
    }
    if (done) toast.success(done);
    await router.invalidate();
    return true;
  };

  const choices = candidates?.filter(
    (u) => !members.some((m) => m.email === u.email),
  );

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">{t('members.hint')}</p>
      {canManage && (
        <form
          className="flex flex-wrap items-start gap-2"
          onSubmit={async (event) => {
            event.preventDefault();
            setPending(true);
            setError(null);
            const result = await addMember(teamId, email, role);
            setPending(false);
            if (!result.success) {
              setError(readableMessage(result.error));
              return;
            }
            toast.success(t('members.added', { email: result.data.email }));
            setEmail('');
            await router.invalidate();
          }}
        >
          <div className="min-w-64 flex-1 space-y-1">
            {choices ? (
              <Choice
                aria-label={t('members.emailLabel')}
                placeholder={
                  choices.length
                    ? t('members.pickUser')
                    : t('members.everyoneIn')
                }
                disabled={choices.length === 0}
                value={email}
                onChange={setEmail}
                options={choices.map((u) => ({
                  value: u.email,
                  label: u.email,
                }))}
              />
            ) : (
              <Input
                type="email"
                placeholder={t('members.emailPlaceholder')}
                aria-label={t('members.emailLabel')}
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                required
              />
            )}
            {error && <p className="text-sm text-destructive">{error}</p>}
          </div>
          <Choice
            aria-label="Role"
            className="w-36"
            value={role}
            onChange={setRole}
            options={roles}
          />
          <Button type="submit" disabled={pending || email === ''}>
            {t('members.add')}
          </Button>
        </form>
      )}
      <DataTable
        head={
          <>
            <ColumnHead>{t('members.email')}</ColumnHead>
            <ColumnHead>{t('members.role')}</ColumnHead>
            {canManage && <ColumnHead className="w-12" />}
          </>
        }
        empty={
          members.length === 0 && <DataTableEmpty title={t('members.empty')} />
        }
      >
        {members.map((member) => (
          <DataRow key={member.user_id}>
            <Cell className="font-medium">{member.email}</Cell>
            <Cell>
              {canManage ? (
                <Choice
                  aria-label={t('members.roleOf', { email: member.email })}
                  className="h-8 w-36"
                  value={member.role}
                  onChange={(next) =>
                    run(
                      () => updateMember(teamId, member.user_id, next),
                      t('members.roleUpdated'),
                    )
                  }
                  options={roles}
                />
              ) : (
                roles.find((r) => r.value === member.role)?.label
              )}
            </Cell>
            {canManage && (
              <ActionsCell>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t('members.remove', { email: member.email })}
                  onClick={() =>
                    run(
                      () => removeMember(teamId, member.user_id),
                      t('members.removed', { email: member.email }),
                    )
                  }
                >
                  <Trash2 />
                </Button>
              </ActionsCell>
            )}
          </DataRow>
        ))}
      </DataTable>
    </div>
  );
}
