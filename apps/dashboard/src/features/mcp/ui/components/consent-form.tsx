import { ShieldAlert } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { AuthorizationInfo } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Button } from '@/shared/ui/components/shadcn/button';
import { decideAuthorization } from '../../api/mutations';
import type { AuthorizationRequest } from '../../api/queries';
import { navigableRedirect } from '../../model/navigable-redirect';

/**
 * Asks the signed-in user whether an assistant may act as them.
 *
 * Both buttons go through the server, which verifies the request again and
 * answers with where to send the browser: the page never builds a redirect
 * itself, so it can only ever send the user to a URL the client registered.
 */
export function ConsentForm({
  request,
  client,
  email,
}: {
  request: AuthorizationRequest;
  client: AuthorizationInfo;
  email: string;
}) {
  const t = useTranslations('oauth');
  const [pending, setPending] = useState(false);

  const decide = async (approve: boolean) => {
    setPending(true);
    const result = await decideAuthorization(request, approve);
    if (result.success && navigableRedirect(result.data.redirect_to)) {
      // Stays pending: the browser is leaving.
      window.location.assign(result.data.redirect_to);
      return;
    }
    setPending(false);
    toast.error(
      t('decideFailed'),
      result.success
        ? undefined
        : { description: readableMessage(result.error) },
    );
  };

  return (
    <div className="space-y-8">
      <div className="space-y-2">
        <h1 className="text-3xl font-bold tracking-tight">
          {t('title', { client: client.client_name })}
        </h1>
        <p className="text-muted-foreground">{t('signedInAs', { email })}</p>
      </div>

      <div className="space-y-3 rounded-lg border bg-muted/40 p-4 text-sm">
        <p className="font-medium">{t('canDo')}</p>
        <ul className="list-disc space-y-1 pl-5 text-muted-foreground">
          <li>{t('canRead')}</li>
          <li>{t('canAct')}</li>
        </ul>
      </div>

      <dl className="space-y-2 text-sm">
        <div>
          <dt className="text-muted-foreground">{t('clientId')}</dt>
          <dd className="font-mono text-xs break-all">{client.client_id}</dd>
        </div>
        <div>
          <dt className="text-muted-foreground">{t('returnsTo')}</dt>
          <dd className="font-mono text-xs">{client.redirect_host}</dd>
        </div>
      </dl>

      <p className="flex gap-2 text-xs text-muted-foreground">
        <ShieldAlert className="size-4 shrink-0" />
        {t('warning')}
      </p>

      <div className="flex gap-3">
        <Button
          className="flex-1"
          disabled={pending}
          onClick={() => decide(true)}
        >
          {t('approve')}
        </Button>
        <Button
          variant="outline"
          className="flex-1"
          disabled={pending}
          onClick={() => decide(false)}
        >
          {t('deny')}
        </Button>
      </div>
    </div>
  );
}
