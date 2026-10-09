import { useFormContext } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import {
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Switch } from '@/shared/ui/components/shadcn/switch';
import { PanelSection } from '@/shared/ui/components/side-panel';

interface BodyValues {
  log_bodies: boolean;
  body_retention_days: string;
}

/**
 * Whether the key keeps each request's content and for how long, as a
 * folded `SidePanel` section; read from the surrounding form.
 */
export function BodySection() {
  const t = useTranslations('keys.form.content');
  const { control, watch, formState } = useFormContext<BodyValues>();
  const on = watch('log_bodies');
  const days = watch('body_retention_days');
  const summary = !on
    ? t('off')
    : days
      ? t('days', { count: Number(days) })
      : t('forever');
  return (
    <PanelSection
      title={t('title')}
      summary={summary}
      forceOpen={Boolean(formState.errors.body_retention_days)}
    >
      <div className="space-y-4">
        <FormField
          control={control}
          name="log_bodies"
          render={({ field }) => (
            <FormItem className="flex items-start justify-between gap-4">
              <div className="space-y-0.5">
                <FormLabel>{t('keep')}</FormLabel>
                <FormDescription>{t('keepHint')}</FormDescription>
              </div>
              <FormControl>
                <Switch
                  checked={field.value}
                  onCheckedChange={field.onChange}
                />
              </FormControl>
            </FormItem>
          )}
        />
        <FormField
          control={control}
          name="body_retention_days"
          render={({ field }) => (
            <FormItem>
              <FormLabel className="text-xs">{t('retention')}</FormLabel>
              <FormControl>
                <Input
                  inputMode="numeric"
                  placeholder={t('forever')}
                  disabled={!on}
                  {...field}
                />
              </FormControl>
              <FormDescription>{t('retentionHint')}</FormDescription>
              <FormMessage />
            </FormItem>
          )}
        />
      </div>
    </PanelSection>
  );
}
