import { createFileRoute, redirect } from '@tanstack/react-router';
import { session } from '@/shared/api/session';
import { OutageScreen } from '@/shared/ui/components/status-screens';

export const Route = createFileRoute('/')({
  beforeLoad: async () => {
    const answer = await session.ensure();
    if (answer.state === 'anonymous') throw redirect({ to: '/login' });
    if (answer.state === 'authenticated') throw redirect({ to: '/keys' });
    return { error: answer.error };
  },
  component: function Outage() {
    const { error } = Route.useRouteContext();
    return <OutageScreen error={error} />;
  },
});
