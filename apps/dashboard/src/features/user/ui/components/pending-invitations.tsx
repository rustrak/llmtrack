import {
  Avatar,
  Button,
  CloseIcon,
  CopyIcon,
  MailIcon,
  Tag,
  Tooltip,
} from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Invitation } from '@/shared/api/schemas';
import { copyToClipboard } from '@/shared/lib/clipboard';
import { readableMessage } from '@/shared/lib/form-errors';
import {
  SettingSection,
  SettingsEmpty,
  SettingsListItem,
} from '@/shared/ui/components/setting-row';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { revokeInvitation } from '../../api/mutations';
import { inviteLink, isOpen } from '../../model/invitation';

/** Invitations not yet accepted, with their link to copy again or revoke. */
export function PendingInvitations({
  invitations,
}: {
  invitations: Invitation[];
}) {
  const t = useTranslations('users');
  const tRoles = useTranslations('roles');
  const money = useMoney();
  const router = useRouter();
  const pending = invitations.filter((i) => i.status === 'pending');

  const copy = async (invite: Invitation) => {
    const link = inviteLink(window.location.origin, invite.token);
    if (await copyToClipboard(link)) toast.success(t('invite.linkCopied'));
    else toast.info(t('invite.copyLink'), { description: link });
  };
  const revoke = async (invite: Invitation) => {
    const result = await revokeInvitation(invite.token);
    if (!result.success) toast.error(readableMessage(result.error));
    else toast.success(t('invite.revoked', { email: invite.email }));
    await router.invalidate();
  };

  return (
    <SettingSection title={t('invite.pending')}>
      {pending.length === 0 ? (
        <SettingsEmpty
          icon={<MailIcon size="2xl" />}
          title={t('invite.none')}
        />
      ) : (
        pending.map((invite) => {
          const revokeLabel = t('invite.revokeOf', { email: invite.email });
          return (
            <SettingsListItem
              key={invite.token}
              leading={<Avatar name={invite.email} />}
              title={
                <span className="flex min-w-0 items-center gap-2">
                  <span className="truncate">{invite.email}</span>
                  <Tag
                    variant="soft"
                    tone={invite.role === 'admin' ? 'brand' : 'neutral'}
                  >
                    {tRoles(invite.role)}
                  </Tag>
                </span>
              }
              description={
                isOpen(invite)
                  ? t('invite.expires', {
                      date: money.dateTime(invite.expires_at),
                    })
                  : t('invite.expired')
              }
              actions={
                <>
                  <Button
                    size="sm"
                    icon={CopyIcon}
                    onClick={() => copy(invite)}
                  >
                    {t('invite.copy')}
                  </Button>
                  <Tooltip content={revokeLabel}>
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={CloseIcon}
                      aria-label={revokeLabel}
                      onClick={() => revoke(invite)}
                    />
                  </Tooltip>
                </>
              }
            />
          );
        })
      )}
    </SettingSection>
  );
}
