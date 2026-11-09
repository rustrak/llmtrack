import { Button, SecurityIcon, Text } from '@rustrak/ui';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { AuthorizationInfo } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
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
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h1 className="text-page-title text-fg">
          {t('title', { client: client.client_name })}
        </h1>
        <Text render={<p />} tone="muted">
          {t('signedInAs', { email })}
        </Text>
      </div>

      <section className="flex flex-col gap-2 rounded-2xl border border-border-subtle bg-surface p-5">
        <h2 className="text-card-title text-fg">{t('canDo')}</h2>
        <ul className="flex list-disc flex-col gap-1 pl-5 text-body text-fg-secondary">
          <li>{t('canRead')}</li>
          <li>{t('canAct')}</li>
        </ul>
      </section>

      <dl className="flex flex-col gap-3">
        <div className="flex flex-col gap-0.5">
          <dt className="text-label text-fg-secondary">{t('clientId')}</dt>
          <dd className="break-all font-mono text-mono-sm text-fg">
            {client.client_id}
          </dd>
        </div>
        <div className="flex flex-col gap-0.5">
          <dt className="text-label text-fg-secondary">{t('returnsTo')}</dt>
          <dd className="font-mono text-mono-sm text-fg">
            {client.redirect_host}
          </dd>
        </div>
      </dl>

      <p className="flex gap-2 text-hint text-fg-subtle">
        <SecurityIcon
          size="sm"
          className="mt-px shrink-0 text-sev-warning-fg"
        />
        {t('warning')}
      </p>

      <div className="flex gap-3">
        <Button
          variant="primary"
          size="lg"
          className="flex-1"
          disabled={pending}
          onClick={() => decide(true)}
        >
          {t('approve')}
        </Button>
        <Button
          variant="secondary"
          size="lg"
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
