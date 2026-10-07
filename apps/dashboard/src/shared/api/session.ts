import { z } from 'zod';
import { type ApiError, request } from './http';
import { type User, userSchema } from './schemas';

/**
 * Three answers, not two: `unavailable` (the server did not answer) must not
 * be read as `anonymous`, or an outage bounces every reader to the login page
 * and back in a loop.
 */
export type CurrentUser =
  | { state: 'authenticated'; user: User }
  | { state: 'anonymous' }
  | { state: 'unavailable'; error: ApiError };

export async function getCurrentUser(): Promise<CurrentUser> {
  const result = await request(
    'GET',
    '/auth/me',
    z.object({ user: userSchema }),
  );
  if (result.success) return { state: 'authenticated', user: result.data.user };
  if (result.error.kind === 'unauthenticated') return { state: 'anonymous' };
  return { state: 'unavailable', error: result.error };
}

export interface SessionStore {
  /** The answer, asking the server only if nothing is known or in flight. */
  ensure(): Promise<CurrentUser>;
  peek(): CurrentUser | undefined;
  /** After login: the answer is known without asking. */
  set(session: CurrentUser): void;
  /** After logout. */
  clear(): void;
}

export function createSessionStore(
  read: () => Promise<CurrentUser>,
): SessionStore {
  let inFlight: Promise<CurrentUser> | null = null;
  let settled: CurrentUser | undefined;

  return {
    ensure() {
      if (inFlight) return inFlight;
      const pending = read().then((answer) => {
        settled = answer;
        // An outage is not an answer worth keeping: ask again next time.
        if (answer.state === 'unavailable' && inFlight === pending) {
          inFlight = null;
        }
        return answer;
      });
      inFlight = pending;
      return pending;
    },
    peek: () => settled,
    set(answer) {
      settled = answer;
      inFlight = Promise.resolve(answer);
    },
    clear() {
      settled = { state: 'anonymous' };
      inFlight = Promise.resolve(settled);
    },
  };
}

export const session = createSessionStore(getCurrentUser);
