import type { LabelColor, LabelRef } from '@/shared/api/schemas';
import { cn } from '@/shared/lib/utils';

/** Each palette colour, spelled out so Tailwind keeps the classes. */
const TONES: Record<LabelColor, { badge: string; dot: string }> = {
  gray: { badge: 'bg-label-gray/12 text-label-gray', dot: 'bg-label-gray' },
  red: { badge: 'bg-label-red/12 text-label-red', dot: 'bg-label-red' },
  orange: {
    badge: 'bg-label-orange/12 text-label-orange',
    dot: 'bg-label-orange',
  },
  amber: { badge: 'bg-label-amber/12 text-label-amber', dot: 'bg-label-amber' },
  green: { badge: 'bg-label-green/12 text-label-green', dot: 'bg-label-green' },
  teal: { badge: 'bg-label-teal/12 text-label-teal', dot: 'bg-label-teal' },
  blue: { badge: 'bg-label-blue/12 text-label-blue', dot: 'bg-label-blue' },
  violet: {
    badge: 'bg-label-violet/12 text-label-violet',
    dot: 'bg-label-violet',
  },
  pink: { badge: 'bg-label-pink/12 text-label-pink', dot: 'bg-label-pink' },
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
        'size-2.5 shrink-0 rounded-pill',
        TONES[color].dot,
        className,
      )}
    />
  );
}

/** A label as a soft tag: the dot, the name, tinted with the label's hue. */
export function LabelBadge({
  label,
}: {
  label: Pick<LabelRef, 'name' | 'color'>;
}) {
  return (
    <span
      className={cn(
        'inline-flex h-5 max-w-48 items-center gap-1.5 rounded-sm px-1.5 text-tag',
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
