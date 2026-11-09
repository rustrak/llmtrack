import { createFileRoute, redirect } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import {
  type AuthorizationRequest,
  getAuthorization,
} from '@/features/mcp/api/queries';
import { ConsentForm } from '@/features/mcp/ui/components/consent-form';
import { returnPath } from '@/shared/api/return-path';
import { session } from '@/shared/api/session';
import { translator } from '@/shared/i18n/intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { OutageScreen } from '@/shared/ui/components/status-screens';
import { AuthHeading, AuthScreen } from '../-components/auth-screen';

const text = (value: unknown) => (typeof value === 'string' ? value : '');
const optional = (value: unknown) =>
  typeof value === 'string' ? value : undefined;

/**
 * `/oauth/authorize`, the authorization endpoint the server advertises: where
 * an assistant such as claude.ai sends the user to approve its access to
 * `/mcp`.
 *
 * Outside `_authenticated`, so the consent page stands on its own without the
 * dashboard around it. It guards itself the same way: a signed-out visitor
 * leaves the address for the login form to resume.
 */
export const Route = createFileRoute('/oauth/authorize')({
  validateSearch: (search: Record<string, unknown>): AuthorizationRequest => ({
    response_type: text(search.response_type),
    client_id: text(search.client_id),
    redirect_uri: text(search.redirect_uri),
    code_challenge: text(search.code_challenge),
    code_challenge_method: text(search.code_challenge_method),
    state: optional(search.state),
    resource: optional(search.resource),
  }),
  beforeLoad: async ({ location }) => {
    const answer = await session.ensure();
    if (answer.state === 'anonymous') {
      returnPath.remember(location.href);
      throw redirect({ to: '/login' });
    }
    return { answer };
  },
  loaderDeps: ({ search }) => search,
  // A request the server refuses is this page's own answer, said in place of
  // the consent form, so the result is not unwrapped.
  loader: ({ deps, context: { answer } }) =>
    answer.state === 'authenticated' ? getAuthorization(deps) : null,
  head: () => ({ meta: [{ title: translator('oauth')('meta.title') }] }),
  component: AuthorizePage,
});

function AuthorizePage() {
  const t = useTranslations('oauth');
  const { answer } = Route.useRouteContext();
  const request = Route.useSearch();
  const client = Route.useLoaderData();

  if (answer.state === 'unavailable') {
    return <OutageScreen error={answer.error} />;
  }

  return (
    <AuthScreen>
      {client?.success && answer.state === 'authenticated' ? (
        <ConsentForm
          request={request}
          client={client.data}
          email={answer.user.email}
        />
      ) : (
        <div className="flex flex-col gap-3">
          <AuthHeading title={t('failedTitle')}>
            {t('failedDescription')}
          </AuthHeading>
          {client && !client.success && (
            <p className="font-mono text-mono-sm text-fg-subtle">
              {readableMessage(client.error)}
            </p>
          )}
        </div>
      )}
    </AuthScreen>
  );
}
