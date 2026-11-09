import { AgentsIcon, Avatar, Button } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { OAuthGrant } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import {
  SettingsEmpty,
  SettingsListItem,
} from '@/shared/ui/components/setting-row';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { revokeGrant } from '../../api/mutations';

/** The host a client is published from, which is what a reader recognises. */
function clientHost(clientId: string): string {
  try {
    return new URL(clientId).host;
  } catch {
    return clientId;
  }
}

/**
 * The assistants connected to the reader's account by signing in, one row
 * each, and the one control that matters for each: cutting it off.
 */
export function ConnectedApps({ grants }: { grants: OAuthGrant[] }) {
  const t = useTranslations('settings');
  const money = useMoney();
  const router = useRouter();
  const [pending, setPending] = useState<number | null>(null);

  const revoke = async (grant: OAuthGrant) => {
    setPending(grant.id);
    const result = await revokeGrant(grant.id);
    setPending(null);
    if (!result.success) {
      toast.error(t('mcp.connected.revokeFailed'), {
        description: readableMessage(result.error),
      });
      return;
    }
    toast.success(t('mcp.connected.revoked', { name: grant.client_name }));
    await router.invalidate();
  };

  if (grants.length === 0) {
    return (
      <SettingsEmpty
        icon={<AgentsIcon size="2xl" />}
        title={t('mcp.connected.empty')}
      />
    );
  }

  return grants.map((grant) => (
    <SettingsListItem
      key={grant.id}
      leading={<Avatar name={grant.client_name} shape="square" />}
      title={grant.client_name}
      description={
        <span className="font-mono">{clientHost(grant.client_id)}</span>
      }
      meta={t('mcp.connected.since', { date: money.date(grant.created_at) })}
      actions={
        <Button
          variant="danger"
          size="sm"
          loading={pending === grant.id}
          disabled={pending !== null}
          onClick={() => revoke(grant)}
          aria-label={t('mcp.connected.revokeLabel', {
            name: grant.client_name,
          })}
        >
          {t('mcp.connected.revoke')}
        </Button>
      }
    />
  ));
}
