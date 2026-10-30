/**
 * Where a signed-out reader was going, held across a sign-in. A page that
 * sends the reader to `/login` leaves its address in this tab's
 * `sessionStorage`, and the login form resumes it.
 *
 * Touching `sessionStorage` throws where the browser refuses it (private
 * mode); signing in then lands on the default page, as it did before.
 */
const KEY = 'llmtrack.returnPath';

export const returnPath = {
  remember(href: string) {
    try {
      sessionStorage.setItem(KEY, href);
    } catch {
      // Refused: see above.
    }
  },
  /** Reads the stored address once, if it is one the app may follow. */
  take(): string | undefined {
    try {
      const stored = sessionStorage.getItem(KEY);
      sessionStorage.removeItem(KEY);
      return resumablePath(stored);
    } catch {
      return undefined;
    }
  },
};

const ORIGIN = 'http://llmtrack.invalid';

/**
 * The stored address, if it is a path on this origin, as the URL parser read
 * it. `//host/x`, `/\host/x` and `/.//host/x` all end up on another host,
 * and following any of them would make the login form an open redirect.
 */
export function resumablePath(
  stored: string | null | undefined,
): string | undefined {
  if (!stored?.startsWith('/')) return undefined;
  try {
    const url = new URL(stored, ORIGIN);
    if (url.origin !== ORIGIN) return undefined;
    const path = url.pathname + url.search + url.hash;
    return /^\/(?![/\\])/.test(path) ? path : undefined;
  } catch {
    return undefined;
  }
}
