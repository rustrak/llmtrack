import { useFormContext } from 'react-hook-form';
import { useFormatter, useTranslations } from 'use-intl';
import { BUDGET_PERIODS } from '@/shared/lib/limits';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { Choice } from './choice';
import {
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from './shadcn/form';
import { Input } from './shadcn/input';
import { PanelSection } from './side-panel';

interface LimitValues {
  max_budget_usd: string;
  budget_duration: string;
  rpm_limit: string;
  tpm_limit: string;
  max_parallel_requests: string;
}

/**
 * Budget and rate limits as two folded `SidePanel` sections, each showing
 * what it is set to; read from the surrounding form.
 */
export function LimitSections({ budgetHint }: { budgetHint?: string }) {
  const t = useTranslations('limits');
  const money = useMoney();
  const format = useFormatter();
  const { control, watch, formState } = useFormContext<LimitValues>();
  const values = watch();
  const errors = formState.errors;
  const periodLabels: Record<(typeof BUDGET_PERIODS)[number], string> = {
    none: t('periodNone'),
    '1d': t('periodDaily'),
    '7d': t('periodWeekly'),
    '30d': t('periodMonthly'),
  };

  const amount = Number(values.max_budget_usd);
  const budgetSummary =
    values.max_budget_usd && Number.isFinite(amount)
      ? [
          money.local(amount),
          values.budget_duration !== 'none' &&
            periodLabels[values.budget_duration as keyof typeof periodLabels],
        ]
          .filter(Boolean)
          .join(' · ')
      : t('noLimit');
  const n = (value: string) => format.number(Number(value));
  const rateSummary =
    [
      values.rpm_limit && t('rpmShort', { n: n(values.rpm_limit) }),
      values.tpm_limit && t('tpmShort', { n: n(values.tpm_limit) }),
      values.max_parallel_requests &&
        t('parallelShort', { n: n(values.max_parallel_requests) }),
    ]
      .filter(Boolean)
      .join(' · ') || t('noLimit');

  const limitInput = (
    name: 'rpm_limit' | 'tpm_limit' | 'max_parallel_requests',
    label: string,
  ) => (
    <FormField
      control={control}
      name={name}
      render={({ field }) => (
        <FormItem>
          <FormLabel className="text-xs">{label}</FormLabel>
          <FormControl>
            <Input inputMode="numeric" placeholder={t('noLimit')} {...field} />
          </FormControl>
          <FormMessage />
        </FormItem>
      )}
    />
  );

  return (
    <>
      <PanelSection
        title={t('sections.budget')}
        summary={budgetSummary}
        forceOpen={Boolean(errors.max_budget_usd)}
      >
        <div className="grid grid-cols-2 items-start gap-3">
          <FormField
            control={control}
            name="max_budget_usd"
            render={({ field }) => (
              <FormItem>
                <FormLabel className="text-xs">
                  {t('budget', { currency: money.currency })}
                </FormLabel>
                <FormControl>
                  <Input
                    inputMode="decimal"
                    placeholder={t('noLimit')}
                    {...field}
                  />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={control}
            name="budget_duration"
            render={({ field }) => (
              <FormItem>
                <FormLabel className="text-xs">{t('period')}</FormLabel>
                <Choice
                  aria-label={t('period')}
                  value={field.value}
                  onChange={field.onChange}
                  options={BUDGET_PERIODS.map((value) => ({
                    value,
                    label: periodLabels[value],
                  }))}
                />
              </FormItem>
            )}
          />
        </div>
        {budgetHint && (
          <p className="mt-3 text-xs text-muted-foreground">{budgetHint}</p>
        )}
      </PanelSection>
      <PanelSection
        title={t('sections.limits')}
        summary={rateSummary}
        forceOpen={Boolean(
          errors.rpm_limit || errors.tpm_limit || errors.max_parallel_requests,
        )}
      >
        <div className="grid grid-cols-3 items-start gap-3">
          {limitInput('rpm_limit', t('rpmLabel'))}
          {limitInput('tpm_limit', t('tpmLabel'))}
          {limitInput('max_parallel_requests', t('parallelLabel'))}
        </div>
      </PanelSection>
    </>
  );
}
