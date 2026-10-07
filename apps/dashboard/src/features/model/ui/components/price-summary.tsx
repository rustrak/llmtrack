import { useTranslations } from 'use-intl';
import type { Pricing } from '@/shared/api/schemas';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** A price in one line: what a million tokens in and out cost, and extras. */
export function PriceSummary({ pricing }: { pricing: Pricing }) {
  const t = useTranslations('pricing');
  const money = useMoney();
  const parts = [
    t('summary.input', { price: money.usd(pricing.input) }),
    t('summary.output', { price: money.usd(pricing.output) }),
  ];
  if (pricing.cache_read !== undefined) {
    parts.push(
      t('summary.cacheRead', { price: money.usd(pricing.cache_read) }),
    );
  }
  if (pricing.above) {
    parts.push(
      t('summary.above', {
        tokens: money.compact(pricing.above.threshold_tokens),
      }),
    );
  }
  if (pricing.per_image !== undefined) {
    parts.push(t('summary.perImage', { price: money.usd(pricing.per_image) }));
  }
  if (pricing.per_second !== undefined) {
    parts.push(
      t('summary.perSecond', { price: money.usd(pricing.per_second) }),
    );
  }
  return (
    <span className="text-xs text-muted-foreground tabular-nums">
      {parts.join(' · ')}
    </span>
  );
}
