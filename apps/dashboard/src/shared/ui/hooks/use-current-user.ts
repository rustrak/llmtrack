import { useRouteContext } from '@tanstack/react-router';
import type { User } from '@/shared/api/schemas';

/** The signed-in user. Only valid under the `_authenticated` layout. */
export function useCurrentUser(): User {
  const answer = useRouteContext({ from: '/_authenticated' });
  if (answer.state !== 'authenticated') {
    throw new Error('useCurrentUser outside the authenticated layout');
  }
  return answer.user;
}
