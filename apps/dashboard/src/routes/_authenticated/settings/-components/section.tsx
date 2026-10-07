import type { ReactNode } from 'react';

/**
 * One block of a settings page as a row: what it is on the left, its fields
 * on the right at a readable width. Rows are divided by a hairline, the way
 * Stripe and GitHub lay out settings.
 */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="grid gap-x-10 gap-y-4 border-b py-8 first:pt-2 last:border-0 lg:grid-cols-[16rem_minmax(0,1fr)]">
      <div className="space-y-1">
        <h2 className="text-sm font-medium">{title}</h2>
        {description && (
          <p className="text-sm text-muted-foreground">{description}</p>
        )}
      </div>
      <div className="max-w-xl min-w-0 space-y-4">{children}</div>
    </section>
  );
}

/** The name of the settings page being shown, above its rows. */
export function SettingsHeader({
  title,
  subtitle,
}: {
  title: string;
  subtitle: string;
}) {
  return (
    <div className="space-y-1 border-b pb-6">
      <h2 className="text-lg font-semibold tracking-tight">{title}</h2>
      <p className="text-sm text-muted-foreground">{subtitle}</p>
    </div>
  );
}
