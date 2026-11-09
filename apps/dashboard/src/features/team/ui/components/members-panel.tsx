import {
  Avatar,
  Button,
  DeleteIcon,
  Input,
  Menu,
  OverflowIcon,
  Tag,
} from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Member } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
import {
  SettingSection,
  SettingsEmpty,
  SettingsListItem,
} from '@/shared/ui/components/setting-row';
import { TeamsIcon } from '@/shared/ui/icons';
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
    <SettingSection
      title={t('detail.membersTab')}
      description={t('members.hint')}
    >
      {canManage && (
        <form
          className="flex flex-wrap items-start gap-2 border-b border-border-divider px-5 py-3"
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
          <div className="flex min-w-64 flex-1 flex-col gap-1">
            {choices ? (
              <Choice
                aria-label={t('members.emailLabel')}
                size="sm"
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
                size="sm"
                type="email"
                placeholder={t('members.emailPlaceholder')}
                aria-label={t('members.emailLabel')}
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                invalid={error !== null}
                required
              />
            )}
            {error && <p className="text-hint text-danger-fg">{error}</p>}
          </div>
          <Choice
            aria-label={t('members.role')}
            size="sm"
            className="w-36"
            value={role}
            onChange={setRole}
            options={roles}
          />
          <Button
            type="submit"
            variant="primary"
            size="sm"
            loading={pending}
            disabled={email === ''}
          >
            {t('members.add')}
          </Button>
        </form>
      )}
      {members.length === 0 ? (
        <SettingsEmpty
          icon={<TeamsIcon size="2xl" />}
          title={t('members.empty')}
        />
      ) : (
        members.map((member) => (
          <SettingsListItem
            key={member.user_id}
            leading={<Avatar name={member.email} />}
            title={
              <span className="flex min-w-0 items-center gap-2">
                <span className="truncate">{member.email}</span>
                <Tag
                  variant="soft"
                  tone={member.role === 'admin' ? 'brand' : 'neutral'}
                >
                  {roles.find((r) => r.value === member.role)?.label}
                </Tag>
              </span>
            }
            actions={
              canManage && (
                <Menu
                  align="end"
                  trigger={
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={OverflowIcon}
                      aria-label={t('detail.moreActions')}
                    />
                  }
                  actions={[
                    {
                      id: 'role',
                      label: t('members.role'),
                      items: roles.map((r) => ({
                        id: r.value,
                        label: r.label,
                        selected: member.role === r.value,
                        onSelect: () => {
                          if (r.value !== member.role)
                            run(
                              () =>
                                updateMember(teamId, member.user_id, r.value),
                              t('members.roleUpdated'),
                            );
                        },
                      })),
                    },
                    {
                      id: 'remove',
                      label: t('members.remove', { email: member.email }),
                      icon: DeleteIcon,
                      tone: 'danger',
                      separated: true,
                      onSelect: () =>
                        run(
                          () => removeMember(teamId, member.user_id),
                          t('members.removed', { email: member.email }),
                        ),
                    },
                  ]}
                />
              )
            }
          />
        ))
      )}
    </SettingSection>
  );
}
