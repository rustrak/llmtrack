import { useRouter } from '@tanstack/react-router';
import { Bot } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { OAuthGrant } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Button } from '@/shared/ui/components/shadcn/button';
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
 * The assistants connected to the reader's account by signing in, and the
 * one control that matters for each: cutting it off.
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
      <div className="flex items-center gap-3 rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
        <Bot className="size-4" />
        {t('mcp.connected.empty')}
      </div>
    );
  }

  return (
    <ul className="divide-y rounded-xl border bg-card">
      {grants.map((grant) => (
        <li
          key={grant.id}
          className="flex flex-wrap items-center gap-3 px-4 py-3 text-sm"
        >
          <div className="min-w-0 flex-1">
            <p className="font-medium">{grant.client_name}</p>
            <p className="truncate font-mono text-xs text-muted-foreground">
              {clientHost(grant.client_id)}
            </p>
          </div>
          <span className="text-xs text-muted-foreground">
            {t('mcp.connected.since', { date: money.date(grant.created_at) })}
          </span>
          <Button
            variant="outline"
            size="sm"
            disabled={pending !== null}
            onClick={() => revoke(grant)}
            aria-label={t('mcp.connected.revokeLabel', {
              name: grant.client_name,
            })}
          >
            {t('mcp.connected.revoke')}
          </Button>
        </li>
      ))}
    </ul>
  );
}
