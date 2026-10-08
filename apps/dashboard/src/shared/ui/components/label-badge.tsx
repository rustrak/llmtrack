import type { LabelColor, LabelRef } from '@/shared/api/schemas';
import { cn } from '@/shared/lib/utils';

/** Each palette colour, spelled out so Tailwind keeps the classes. */
const TONES: Record<LabelColor, { badge: string; dot: string }> = {
  gray: {
    badge: 'border-zinc-500/30 bg-zinc-500/10 text-zinc-700 dark:text-zinc-300',
    dot: 'bg-zinc-500',
  },
  red: {
    badge: 'border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-300',
    dot: 'bg-red-500',
  },
  orange: {
    badge:
      'border-orange-500/30 bg-orange-500/10 text-orange-700 dark:text-orange-300',
    dot: 'bg-orange-500',
  },
  amber: {
    badge:
      'border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300',
    dot: 'bg-amber-500',
  },
  green: {
    badge:
      'border-green-500/30 bg-green-500/10 text-green-700 dark:text-green-300',
    dot: 'bg-green-500',
  },
  teal: {
    badge: 'border-teal-500/30 bg-teal-500/10 text-teal-700 dark:text-teal-300',
    dot: 'bg-teal-500',
  },
  blue: {
    badge: 'border-blue-500/30 bg-blue-500/10 text-blue-700 dark:text-blue-300',
    dot: 'bg-blue-500',
  },
  violet: {
    badge:
      'border-violet-500/30 bg-violet-500/10 text-violet-700 dark:text-violet-300',
    dot: 'bg-violet-500',
  },
  pink: {
    badge: 'border-pink-500/30 bg-pink-500/10 text-pink-700 dark:text-pink-300',
    dot: 'bg-pink-500',
  },
};

/** The swatch of a colour, for pickers. */
export function LabelDot({
  color,
  className,
}: {
  color: LabelColor;
  className?: string;
}) {
  return (
    <span
      aria-hidden
      className={cn(
        'size-2.5 shrink-0 rounded-full',
        TONES[color].dot,
        className,
      )}
    />
  );
}

export function LabelBadge({
  label,
}: {
  label: Pick<LabelRef, 'name' | 'color'>;
}) {
  return (
    <span
      className={cn(
        'inline-flex h-5 max-w-48 items-center gap-1.5 rounded-md border px-1.5 text-xs font-medium',
        TONES[label.color].badge,
      )}
    >
      <LabelDot color={label.color} className="size-1.5" />
      <span className="truncate">{label.name}</span>
    </span>
  );
}

export function LabelBadges({ labels }: { labels: readonly LabelRef[] }) {
  if (labels.length === 0) return null;
  return (
    <div className="flex max-w-64 flex-wrap gap-1">
      {labels.map((label) => (
        <LabelBadge key={label.id} label={label} />
      ))}
    </div>
  );
}
