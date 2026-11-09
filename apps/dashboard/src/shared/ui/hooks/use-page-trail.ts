import { useEffect, useSyncExternalStore } from 'react';

/** A crumb a page adds after its section's: what it shows, and maybe a way back. */
export interface TrailCrumb {
  label: string;
  href?: string;
  /** An identifier the system chose, set in mono. */
  mono?: boolean;
}

// One trail for the one page on screen; the topbar reads what the page sets.
let trail: TrailCrumb[] = [];
const listeners = new Set<() => void>();

function setTrail(next: TrailCrumb[]) {
  trail = next;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * Names the page in the breadcrumbs, after its section: a release's version,
 * an endpoint's name, the endpoint a transaction belongs to. The section
 * before it becomes the way back. Cleared when the page goes.
 */
export function usePageTrail(crumbs: TrailCrumb[]) {
  const key = JSON.stringify(crumbs);
  useEffect(() => {
    setTrail(JSON.parse(key) as TrailCrumb[]);
    return () => setTrail([]);
  }, [key]);
}

/** The crumbs the page on screen set, for the breadcrumbs to draw. */
export function usePageTrailCrumbs(): TrailCrumb[] {
  return useSyncExternalStore(subscribe, () => trail);
}
