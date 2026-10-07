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
      <p className="text-sm tabular-nums">
        {money.usd(spent)}
        <span className="text-muted-foreground">
          {' / '}
          {budget === null ? t('noLimit') : money.usd(budget)}
        </span>
      </p>
      {share !== null && (
        <div className="h-1 w-full overflow-hidden rounded-full bg-muted">
          <div
            className={cn(
              'h-full rounded-full',
              share >= 0.9 ? 'bg-destructive' : 'bg-primary',
            )}
            style={{ width: `${Math.max(share * 100, 2)}%` }}
          />
        </div>
      )}
    </div>
  );
}
