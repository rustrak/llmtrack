import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { getInvitation } from '@/features/user/api/queries';
import { translator } from '@/shared/i18n/intl';
import { OutageScreen } from '@/shared/ui/components/status-screens';
import { AuthHeading, AuthScreen } from '../-components/auth-screen';
import { AcceptInvitationForm } from './-components/accept-invitation-form';

/** Public: where an invitee sets their password. */
export const Route = createFileRoute('/invite/$token')({
  loader: ({ params }) => getInvitation(params.token),
  head: () => ({ meta: [{ title: translator('invite')('meta.title') }] }),
  component: InvitePage,
});

function InvitePage() {
  const t = useTranslations('invite');
  const result = Route.useLoaderData();
  const { token } = Route.useParams();
  // An outage does not mean the link is dead.
  if (
    !result.success &&
    !['not_found', 'validation'].includes(result.error.kind)
  ) {
    return <OutageScreen error={result.error} />;
  }
  // The server answers only for a pending, unexpired invitation.
  const invitation = result.success ? result.data : null;
  return (
    <AuthScreen>
      {invitation ? (
        <AcceptInvitationForm token={token} email={invitation.email} />
      ) : (
        <div>
          <AuthHeading title={t('unavailableTitle')}>
            {t('unavailableDescription')}
          </AuthHeading>
        </div>
      )}
    </AuthScreen>
  );
}
