import { Checkbox } from '@rustrak/ui';
import { useTranslations } from 'use-intl';
import type { ModelRef } from '@/shared/api/schemas';
import { byName, isPicked, toggle } from '@/shared/lib/models';
import { PickRow } from './label-picker';

/**
 * Picks an allowlist of models, one line per model name: a name with several
 * deployments is allowed whole. Nothing picked means every model the parent
 * allows, which is what the server does with an empty list too.
 */
export function ModelPicker({
  models,
  value,
  onChange,
  emptyMeaning,
}: {
  models: readonly ModelRef[];
  value: number[];
  onChange: (value: number[]) => void;
  emptyMeaning: string;
}) {
  const t = useTranslations('common');
  if (models.length === 0) {
    return (
      <p className="text-hint text-fg-subtle">
        {t('noModelsYet')} {emptyMeaning}
      </p>
    );
  }
  return (
    <div className="space-y-2">
      <div className="max-h-48 overflow-y-auto rounded-lg border border-border-subtle">
        {byName(models).map((model) => (
          <PickRow key={model.id}>
            <Checkbox
              aria-label={model.name}
              checked={isPicked(models, value, model.name)}
              onCheckedChange={(next) =>
                onChange(toggle(models, value, model.name, next))
              }
            />
            <span className="font-mono text-mono text-fg">{model.name}</span>
          </PickRow>
        ))}
      </div>
      <p className="text-hint text-fg-subtle">
        {value.length === 0
          ? emptyMeaning
          : t('selectedCount', {
              count: byName(models).filter((m) =>
                isPicked(models, value, m.name),
              ).length,
            })}
      </p>
    </div>
  );
}
