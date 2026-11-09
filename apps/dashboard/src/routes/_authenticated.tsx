import { AppShell, Topbar, TopbarMenuButton } from '@rustrak/ui';
import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';
import { TimeZoneSync } from '@/features/user/ui/components/time-zone-sync';
import { session } from '@/shared/api/session';
import { OutageScreen } from '@/shared/ui/components/status-screens';
import { TableLabels } from '@/shared/ui/components/table-labels';
import {
  AppBreadcrumbs,
  AppSidebar,
} from './_authenticated/-components/app-frame';

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
    <AppShell
      sidebar={<AppSidebar user={answer.user} />}
      defaultCollapsed={sidebarWasCollapsed()}
      onCollapsedChange={rememberSidebar}
    >
      <TimeZoneSync hasTimeZone={Boolean(answer.user.timezone)} />
      <Topbar menu={<TopbarMenuButton />}>
        <AppBreadcrumbs user={answer.user} />
      </Topbar>
      {/* The sheet's only scroller. Screens that own their scrolling fill it
          with `h-full` and scroll inside. */}
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        <TableLabels>
          <Outlet />
        </TableLabels>
      </div>
    </AppShell>
  );
}

/**
 * Whether the sidebar was left collapsed, read synchronously so the first
 * paint matches what the reader left rather than expanding and snapping shut.
 */
function sidebarWasCollapsed(): boolean {
  return document.cookie
    .split('; ')
    .some((entry) => entry === 'sidebar_state=false');
}

function rememberSidebar(collapsed: boolean) {
  // biome-ignore lint/suspicious/noDocumentCookie: one boolean, read before the first paint; the Cookie Store API is async.
  document.cookie = `sidebar_state=${!collapsed}; path=/; max-age=31536000; SameSite=Lax`;
}
