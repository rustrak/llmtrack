import { useTranslations } from 'use-intl';
import type { ModelRef } from '@/shared/api/schemas';
import { byName, isPicked, toggle } from '@/shared/lib/models';
import { Checkbox } from './shadcn/checkbox';

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
      <p className="text-sm text-muted-foreground">
        {t('noModelsYet')} {emptyMeaning}
      </p>
    );
  }
  return (
    <div className="space-y-2">
      <div className="max-h-48 overflow-y-auto rounded-lg border divide-y">
        {byName(models).map((model) => {
          const checked = isPicked(models, value, model.name);
          return (
            <label
              key={model.id}
              className="flex cursor-pointer items-center gap-3 px-3 py-2 text-sm hover:bg-muted/50"
            >
              <Checkbox
                checked={checked}
                onCheckedChange={(next) =>
                  onChange(toggle(models, value, model.name, next))
                }
              />
              <span className="font-mono text-xs">{model.name}</span>
            </label>
          );
        })}
      </div>
      <p className="text-xs text-muted-foreground">
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
