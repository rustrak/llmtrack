import { Checkbox } from '@rustrak/ui';
import { Link } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import type { LabelRef } from '@/shared/api/schemas';
import { LabelBadge } from './label-badge';

/** Ticks any of the labels admins defined; none is fine. */
export function LabelPicker({
  labels,
  value,
  onChange,
}: {
  labels: readonly LabelRef[];
  value: number[];
  onChange: (value: number[]) => void;
}) {
  const t = useTranslations('common');
  if (labels.length === 0) {
    return (
      <p className="text-hint text-fg-subtle">
        {t('noLabelsYet')}{' '}
        <Link to="/labels" className="text-fg-brand hover:underline">
          {t('createLabels')}
        </Link>
      </p>
    );
  }
  return (
    <div className="max-h-48 overflow-y-auto rounded-lg border border-border-subtle">
      {labels.map((label) => (
        <PickRow key={label.id}>
          <Checkbox
            aria-label={label.name}
            checked={value.includes(label.id)}
            onCheckedChange={(next) =>
              onChange(
                next
                  ? [...value, label.id]
                  : value.filter((id) => id !== label.id),
              )
            }
          />
          <LabelBadge label={label} />
        </PickRow>
      ))}
    </div>
  );
}

/** One line of a tick list: the box and what it picks, the whole row clickable. */
export function PickRow({ children }: { children: React.ReactNode }) {
  return (
    <label className="flex min-h-control-lg cursor-pointer items-center gap-3 border-border-divider border-b px-3 py-1.5 text-control text-fg-secondary last:border-b-0 hover:bg-surface-hover">
      {children}
    </label>
  );
}
