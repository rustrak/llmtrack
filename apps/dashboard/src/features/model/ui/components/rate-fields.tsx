import type { Control } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import { Caption } from '@/shared/ui/components/page';
import {
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
import type { ModelFormValues, RateField } from '../../model/model-form';

function Rate({
  control,
  field,
  label,
}: {
  control: Control<ModelFormValues>;
  field: RateField;
  label: string;
}) {
  return (
    <FormField
      control={control}
      name={`rates.${field}`}
      render={({ field: input }) => (
        <FormItem>
          <FormLabel className="text-xs">{label}</FormLabel>
          <FormControl>
            <Input inputMode="decimal" className="tabular-nums" {...input} />
          </FormControl>
          <FormMessage />
        </FormItem>
      )}
    />
  );
}

/**
 * Own rates: input and output per million tokens up front,
 * cache, reasoning, the long-context tier and per-unit prices behind
 * "advanced". Blank means "not set": cache and reasoning then bill at the
 * input and output rates.
 */
export function RateFields({ control }: { control: Control<ModelFormValues> }) {
  const t = useTranslations('pricing');
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-3">
        <Rate control={control} field="input" label={t('rates.input')} />
        <Rate control={control} field="output" label={t('rates.output')} />
        <Rate
          control={control}
          field="cache_read"
          label={t('rates.cacheRead')}
        />
        <Rate
          control={control}
          field="cache_write"
          label={t('rates.cacheWrite')}
        />
      </div>
      <details className="group rounded-lg border px-3 py-2">
        <summary className="cursor-pointer text-sm font-medium">
          {t('rates.advanced')}
        </summary>
        <div className="space-y-4 pt-3">
          <div className="grid grid-cols-2 gap-3">
            <Rate
              control={control}
              field="cache_write_1h"
              label={t('rates.cacheWrite1h')}
            />
            <Rate
              control={control}
              field="reasoning"
              label={t('rates.reasoning')}
            />
          </div>
          <div className="space-y-2">
            <Caption>{t('rates.longContext')}</Caption>
            <p className="text-xs text-muted-foreground">
              {t('rates.longContextHint')}
            </p>
            <div className="grid grid-cols-2 gap-3">
              <Rate
                control={control}
                field="above_threshold"
                label={t('rates.threshold')}
              />
              <Rate
                control={control}
                field="above_input"
                label={t('rates.input')}
              />
              <Rate
                control={control}
                field="above_output"
                label={t('rates.output')}
              />
              <Rate
                control={control}
                field="above_cache_read"
                label={t('rates.cacheRead')}
              />
            </div>
          </div>
          <div className="space-y-2">
            <Caption>{t('rates.perUnit')}</Caption>
            <div className="grid grid-cols-3 gap-3">
              <Rate
                control={control}
                field="per_image"
                label={t('rates.perImage')}
              />
              <Rate
                control={control}
                field="per_second"
                label={t('rates.perSecond')}
              />
              <Rate
                control={control}
                field="per_million_characters"
                label={t('rates.perMillionCharacters')}
              />
            </div>
          </div>
        </div>
      </details>
    </div>
  );
}
