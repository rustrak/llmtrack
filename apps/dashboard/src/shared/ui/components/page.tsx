import { PageHeader, Page as SheetPage } from '@rustrak/ui';
import type { ReactNode } from 'react';

/**
 * The frame of every screen: the design system's header (title, one line of
 * context, the page's actions, on a rule the width of the sheet) and the
 * page under it. `fill` hands the content the height the header leaves, for a
 * list whose table scrolls on its own; without it the page scrolls as a whole.
 */
export function Page({
  title,
  description,
  actions,
  tabs,
  fill,
  children,
}: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  /** A `TabList`, sitting on the header's rule. */
  tabs?: ReactNode;
  fill?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageHeader
        title={title}
        summary={description ? [description] : undefined}
        actions={actions}
        tabs={tabs}
      />
      <SheetPage scroll={!fill}>{children}</SheetPage>
    </div>
  );
}

/** A small mono heading: a section's or a field group's caption. */
export function Caption({ children }: { children: ReactNode }) {
  return (
    <span className="font-mono text-column text-fg-meta uppercase">
      {children}
    </span>
  );
}
