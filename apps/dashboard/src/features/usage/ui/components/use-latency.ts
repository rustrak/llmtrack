import { useFormatter, useTranslations } from 'use-intl';

/** Milliseconds as "840 ms" or, from a second up, "1.8 s". */
export function useLatency() {
  const t = useTranslations('usage');
  const format = useFormatter();
  return (ms: number) =>
    ms < 1000
      ? t('latency.ms', { value: format.number(Math.round(ms)) })
      : t('latency.s', {
          value: format.number(ms / 1000, { maximumFractionDigits: 1 }),
        });
}
