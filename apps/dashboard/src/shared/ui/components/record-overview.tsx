import { ExternalLinkIcon } from '@rustrak/ui';
import { Link } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import { useTranslations } from 'use-intl';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { ShareBar } from './spend-bar';

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
        className="flex size-11 shrink-0 items-center justify-center rounded-xl border border-border-subtle bg-surface-raised font-mono text-value font-semibold text-fg-secondary [&_svg]:text-fg-subtle"
      >
        {tile}
      </span>
      <span className="truncate">{name}</span>
      {aside && (
        <span className="shrink-0 font-mono text-mono text-fg-subtle">
          {aside}
        </span>
      )}
    </span>
  );
}

/** The line of facts under the title, aligned with the name. */
export function RecordFacts({ children }: { children: ReactNode }) {
  return (
    <span className="flex flex-wrap items-center gap-2 pl-15">{children}</span>
  );
}

/** A quiet fact in the facts line. */
export function Fact({ children }: { children: ReactNode }) {
  return <span className="text-meta text-fg-subtle">{children}</span>;
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
    <section className="flex flex-col gap-5 rounded-2xl border border-border-subtle bg-surface p-5 lg:col-span-2">
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-1">
          <p className="text-card-title text-fg">{label}</p>
          <p className="text-numeric-lg text-fg tabular-nums">{total}</p>
          <p className="text-meta text-fg-subtle tabular-nums">{hint}</p>
        </div>
        {link}
      </header>
      {budget && (
        <div className="space-y-2">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <span className="text-value text-fg tabular-nums">
              {budget.text}
            </span>
            <span className="text-meta text-fg-subtle">{budget.reset}</span>
          </div>
          <ShareBar share={budget.share} className="h-1.5" />
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
    <section className="rounded-2xl border border-border-subtle bg-surface px-5 pt-4 pb-2">
      <h2 className="pb-1 text-card-title text-fg">{title}</h2>
      <dl>
        {rows.map(([label, value]) => (
          <div
            key={label}
            className="flex items-start justify-between gap-4 border-border-divider border-b py-2.5 text-value last:border-b-0"
          >
            <dt className="text-fg-subtle">{label}</dt>
            <dd className="min-w-0 text-right font-medium text-fg tabular-nums">
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
  return <span className="font-normal text-fg-subtle">{children}</span>;
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
      className="flex h-control-sm items-center gap-1.5 rounded-md px-2 text-control text-fg-subtle transition-[background-color,color] duration-fast hover:bg-surface-hover hover:text-fg"
    >
      {label}
      <ExternalLinkIcon size="md" />
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
        <span
          key={m.id}
          className="inline-flex h-5 items-center rounded-sm bg-surface-chip px-1.5 font-mono text-mono-sm text-fg-secondary"
        >
          {m.name}
        </span>
      ))}
    </span>
  );
}
