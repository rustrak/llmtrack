import { Link } from '@tanstack/react-router';
import { ArrowUpRight } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslations } from 'use-intl';
import { cn } from '@/shared/lib/utils';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { Badge } from './shadcn/badge';

/**
 * The pieces of a record's page (a team, a key), so every record reads the
 * same way: a tile and a name, a line of facts, what it spent beside how it
 * is set up. Callers pass their words; nothing here knows the record.
 */

/** Up to two initials, for a tile. */
export function initials(name: string) {
  const words = name.trim().split(/\s+/).filter(Boolean);
  const letters =
    words.length > 1
      ? words[0][0] + words[1][0]
      : (words[0] ?? '?').slice(0, 2);
  return letters.toUpperCase();
}

/** The page title: a tile (initials or an icon) and the record's name. */
export function RecordTitle({
  tile,
  name,
  aside,
}: {
  tile: ReactNode;
  name: string;
  /** Next to the name, quieter: an id, a hint. */
  aside?: ReactNode;
}) {
  return (
    <span className="flex min-w-0 items-center gap-4">
      <span
        aria-hidden
        className="flex size-12 shrink-0 items-center justify-center rounded-xl border bg-muted/60 font-mono text-base font-semibold [&_svg]:size-5 [&_svg]:text-muted-foreground"
      >
        {tile}
      </span>
      <span className="truncate">{name}</span>
      {aside && (
        <span className="shrink-0 font-mono text-sm font-normal text-muted-foreground">
          {aside}
        </span>
      )}
    </span>
  );
}

/** The line of facts under the title, aligned with the name. */
export function RecordFacts({ children }: { children: ReactNode }) {
  return (
    <span className="flex flex-wrap items-center gap-2 pl-16">{children}</span>
  );
}

/** A quiet fact in the facts line. */
export function Fact({ children }: { children: ReactNode }) {
  return <span className="text-xs text-muted-foreground">{children}</span>;
}

/**
 * What the record spent over a period, how close it is to its budget, and
 * the chart under it.
 */
export function SpendSummary({
  label,
  total,
  hint,
  link,
  budget,
  chart,
}: {
  label: string;
  total: string;
  hint: string;
  /** Where to look further, top right. */
  link?: ReactNode;
  budget?: {
    /** e.g. "$4.20 of $10.00 budget". */
    text: string;
    /** e.g. "Resets Oct 31". */
    reset: string;
    /** Used, 0 to 1. */
    share: number;
  };
  chart: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-5 rounded-xl border bg-card p-5 lg:col-span-2">
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-1">
          <p className="text-xs font-medium text-muted-foreground">{label}</p>
          <p className="text-3xl font-semibold tracking-tight tabular-nums">
            {total}
          </p>
          <p className="text-xs text-muted-foreground tabular-nums">{hint}</p>
        </div>
        {link}
      </header>
      {budget && (
        <div className="space-y-2">
          <div className="flex flex-wrap items-baseline justify-between gap-2 text-sm">
            <span className="tabular-nums">{budget.text}</span>
            <span className="text-xs text-muted-foreground">
              {budget.reset}
            </span>
          </div>
          <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
            <div
              className={cn(
                'h-full rounded-full transition-[width]',
                budget.share >= 0.9 ? 'bg-destructive' : 'bg-primary',
              )}
              style={{ width: `${Math.max(budget.share * 100, 1)}%` }}
            />
          </div>
        </div>
      )}
      {chart}
    </section>
  );
}

/** How the record is set up, label by value, as Stripe shows a customer. */
export function DetailsList({
  title,
  rows,
}: {
  title: string;
  rows: [label: string, value: ReactNode][];
}) {
  return (
    <section className="rounded-xl border bg-card px-5 py-4">
      <h2 className="pb-1 text-sm font-medium">{title}</h2>
      <dl className="divide-y">
        {rows.map(([label, value]) => (
          <div
            key={label}
            className="flex items-start justify-between gap-4 py-2.5 text-sm"
          >
            <dt className="text-muted-foreground">{label}</dt>
            <dd className="min-w-0 text-right font-medium tabular-nums">
              {value}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

/** A value that means "nothing set", in the details list. */
export function Unset({ children }: { children: ReactNode }) {
  return <span className="font-normal text-muted-foreground">{children}</span>;
}

/** Where to read more about the record's spend: Usage, filtered to it. */
export function UsageLink({
  search,
  label,
}: {
  search: { team?: number; key?: number };
  label: string;
}) {
  return (
    <Link
      to="/usage"
      search={search}
      className="flex items-center gap-1 rounded-md px-2 py-1 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
    >
      {label}
      <ArrowUpRight className="size-4" />
    </Link>
  );
}

/** Budget and rate limits as details rows: teams and keys share them. */
export function useLimitRows(record: {
  max_budget_usd: number | null;
  budget_duration: string | null;
  rpm_limit: number | null;
  tpm_limit: number | null;
  max_parallel_requests: number | null;
}): [string, ReactNode][] {
  const tLimits = useTranslations('limits');
  const money = useMoney();
  const none = <Unset>{tLimits('noLimit')}</Unset>;
  const limit = (value: number | null) =>
    value === null ? none : money.compact(value);
  const periods: Record<string, string> = {
    '1d': tLimits('periodDaily'),
    '7d': tLimits('periodWeekly'),
    '30d': tLimits('periodMonthly'),
  };
  return [
    [
      tLimits('budgetShort'),
      record.max_budget_usd === null ? none : money.usd(record.max_budget_usd),
    ],
    [
      tLimits('period'),
      record.budget_duration
        ? (periods[record.budget_duration] ?? record.budget_duration)
        : tLimits('periodNone'),
    ],
    [tLimits('rpmLabel'), limit(record.rpm_limit)],
    [tLimits('tpmLabel'), limit(record.tpm_limit)],
    [tLimits('parallelLabel'), limit(record.max_parallel_requests)],
  ];
}

/** Model names as badges, right-aligned for a details row. */
export function ModelList({
  models,
}: {
  models: { id: number; name: string }[];
}) {
  return (
    <span className="flex max-w-56 flex-wrap justify-end gap-1">
      {models.map((m) => (
        <Badge key={m.id} variant="outline" className="font-mono">
          {m.name}
        </Badge>
      ))}
    </span>
  );
}
