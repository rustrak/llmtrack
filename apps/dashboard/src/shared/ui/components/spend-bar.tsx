import { useTranslations } from 'use-intl';
import { budgetShare } from '@/shared/lib/format';
import { cn } from '@/shared/lib/utils';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** Spend against budget: the number, and a bar that turns red near the cap. */
export function SpendBar({
  spent,
  budget,
}: {
  spent: number;
  budget: number | null;
}) {
  const t = useTranslations('common');
  const money = useMoney();
  const share = budgetShare(spent, budget);
  return (
    <div className="min-w-32 space-y-1">
      <p className="text-body text-fg tabular-nums">
        {money.usd(spent)}
        <span className="text-fg-subtle">
          {' / '}
          {budget === null ? t('noLimit') : money.usd(budget)}
        </span>
      </p>
      {share !== null && <ShareBar share={share} />}
    </div>
  );
}

/** A thin bar for a share of a budget, red from 90 %. */
export function ShareBar({
  share,
  className,
}: {
  share: number;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'h-1 w-full overflow-hidden rounded-pill bg-surface-chip',
        className,
      )}
    >
      <div
        className={cn(
          'h-full rounded-pill transition-[width] duration-moderate ease-standard',
          share >= 0.9 ? 'bg-danger' : 'bg-surface-brand',
        )}
        style={{ width: `${Math.max(share * 100, 2)}%` }}
      />
    </div>
  );
}
