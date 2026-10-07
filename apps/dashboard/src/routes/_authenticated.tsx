import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';
import { TimeZoneSync } from '@/features/user/ui/components/time-zone-sync';
import { session } from '@/shared/api/session';
import { Separator } from '@/shared/ui/components/shadcn/separator';
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from '@/shared/ui/components/shadcn/sidebar';
import { OutageScreen } from '@/shared/ui/components/status-screens';
import { AppSidebar } from './_authenticated/-components/app-sidebar';
import { Crumbs } from './_authenticated/-components/crumbs';

/** The one auth gate: every signed-in page sits under this layout. */
export const Route = createFileRoute('/_authenticated')({
  beforeLoad: async () => {
    const answer = await session.ensure();
    if (answer.state === 'anonymous') throw redirect({ to: '/login' });
    return answer;
  },
  component: AuthenticatedLayout,
});

function AuthenticatedLayout() {
  const answer = Route.useRouteContext();
  if (answer.state !== 'authenticated') {
    return answer.state === 'unavailable' ? (
      <OutageScreen error={answer.error} />
    ) : null;
  }
  return (
    <SidebarProvider>
      <TimeZoneSync hasTimeZone={Boolean(answer.user.timezone)} />
      <AppSidebar user={answer.user} />
      {/* Fixed to the viewport: pages scroll inside, so a list page can
          hand the leftover height to its table. */}
      <SidebarInset className="h-svh overflow-hidden md:h-[calc(100svh-1rem)]">
        <header className="flex h-12 shrink-0 items-center gap-2 border-b px-3">
          <SidebarTrigger />
          <Separator orientation="vertical" className="my-3.5" />
          <Crumbs user={answer.user} />
        </header>
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          <Outlet />
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
