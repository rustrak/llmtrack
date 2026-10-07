import { useRouter } from '@tanstack/react-router';
import { Copy, Mail, X } from 'lucide-react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Invitation } from '@/shared/api/schemas';
import { copyToClipboard } from '@/shared/lib/clipboard';
import { readableMessage } from '@/shared/lib/form-errors';
import { Caption } from '@/shared/ui/components/page';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { Button } from '@/shared/ui/components/shadcn/button';
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
    <div className="space-y-2">
      <Caption>{t('invite.pending')}</Caption>
      {pending.length === 0 ? (
        <div className="flex items-center gap-3 rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
          <Mail className="size-4" />
          {t('invite.none')}
        </div>
      ) : (
        <ul className="divide-y rounded-xl border bg-card">
          {pending.map((invite) => (
            <li
              key={invite.token}
              className="flex flex-wrap items-center gap-3 px-4 py-3 text-sm"
            >
              <span className="font-medium">{invite.email}</span>
              <Badge variant="outline">{tRoles(invite.role)}</Badge>
              <span className="flex-1 text-xs text-muted-foreground">
                {isOpen(invite)
                  ? t('invite.expires', {
                      date: money.dateTime(invite.expires_at),
                    })
                  : t('invite.expired')}
              </span>
              <Button variant="outline" size="sm" onClick={() => copy(invite)}>
                <Copy />
                {t('invite.copy')}
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t('invite.revokeOf', { email: invite.email })}
                onClick={() => revoke(invite)}
              >
                <X />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
