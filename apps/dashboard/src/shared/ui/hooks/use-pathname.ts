import { useLocation } from '@tanstack/react-router';

/**
 * The current path, without the query string.
 *
 * The three call sites are all the same shape: a settings nav deciding which
 * row is the active one. `useLocation` with a selector rather than the whole
 * location, so a change to `?page=2` does not re-render a sidebar that cannot
 * see it.
 */
export function usePathname(): string {
  return useLocation({ select: (location) => location.pathname });
}
