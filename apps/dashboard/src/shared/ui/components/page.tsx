import type { ReactNode } from 'react';
import { cn } from '@/shared/lib/utils';

/**
 * The frame of every screen: title, one line of context, page actions.
 * `fill` gives the content the height the header leaves, for a list whose
 * table scrolls on its own; without it the page scrolls as a whole.
 */
export function Page({
  title,
  description,
  actions,
  fill,
  children,
}: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  fill?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={cn(
        'flex w-full flex-col gap-6 px-4 py-6 md:px-8 md:py-8',
        fill && 'min-h-0 flex-1',
      )}
    >
      <header className="flex shrink-0 flex-wrap items-start justify-between gap-x-6 gap-y-4">
        <div className="min-w-0 space-y-1.5">
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
          {description && (
            <p className="max-w-2xl text-sm text-muted-foreground">
              {description}
            </p>
          )}
        </div>
        {actions && (
          <div className="flex shrink-0 flex-wrap items-center gap-2">
            {actions}
          </div>
        )}
      </header>
      {children}
    </div>
  );
}

/** A small uppercase label, the dashboard's section and field caption. */
export function Caption({ children }: { children: ReactNode }) {
  return (
    <span className="text-xs font-bold uppercase tracking-widest text-muted-foreground">
      {children}
    </span>
  );
}
