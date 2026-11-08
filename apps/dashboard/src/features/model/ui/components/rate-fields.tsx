import { ChevronRightIcon, Input } from '@rustrak/ui';
import type { Control } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import {
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/form';
import { Caption } from '@/shared/ui/components/page';
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
          <FormLabel>{label}</FormLabel>
          <FormControl>
            <Input
              inputMode="decimal"
              className="font-mono tabular-nums"
              {...input}
            />
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
      <details className="group/adv rounded-xl border border-border-subtle bg-surface-raised open:bg-transparent">
        <summary className="flex h-control-lg cursor-pointer list-none items-center gap-2 rounded-xl px-3 text-control font-medium text-fg select-none hover:bg-surface-hover [&::-webkit-details-marker]:hidden">
          <ChevronRightIcon
            size="sm"
            className="text-fg-ghost transition-[rotate] duration-fast ease-standard group-open/adv:rotate-90"
          />
          {t('rates.advanced')}
        </summary>
        <div className="space-y-4 border-border-subtle border-t px-3 pt-3 pb-4">
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
            <p className="text-hint text-fg-subtle">
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
