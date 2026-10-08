import { Link } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import type { LabelRef } from '@/shared/api/schemas';
import { LabelBadge } from './label-badge';
import { Checkbox } from './shadcn/checkbox';

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
      <p className="text-sm text-muted-foreground">
        {t('noLabelsYet')}{' '}
        <Link to="/labels" className="underline underline-offset-4">
          {t('createLabels')}
        </Link>
      </p>
    );
  }
  return (
    <div className="max-h-48 divide-y overflow-y-auto rounded-lg border">
      {labels.map((label) => (
        <label
          key={label.id}
          className="flex cursor-pointer items-center gap-3 px-3 py-2 text-sm hover:bg-muted/50"
        >
          <Checkbox
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
        </label>
      ))}
    </div>
  );
}
