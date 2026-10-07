import { createRouter } from '@tanstack/react-router';
import {
  ErrorScreen,
  NotFoundScreen,
} from '@/shared/ui/components/status-screens';
import { routeTree } from './routeTree.gen';

export function createAppRouter() {
  return createRouter({
    routeTree,
    defaultPendingMs: 300,
    defaultPendingMinMs: 300,
    defaultPreload: 'intent',
    defaultNotFoundComponent: NotFoundScreen,
    defaultErrorComponent: ({ error }) => <ErrorScreen error={error} />,
    scrollRestoration: true,
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
