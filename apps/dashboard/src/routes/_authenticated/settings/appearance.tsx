import { createFileRoute, redirect } from '@tanstack/react-router';

/** Moved to `/settings/preferences`; kept so links still land. */
export const Route = createFileRoute('/_authenticated/settings/appearance')({
  beforeLoad: () => {
    throw redirect({ to: '/settings/preferences' });
  },
});
