/** USD is what providers bill; the others are display currencies. */
export const CURRENCIES = ['USD', 'EUR'] as const;
export type Currency = (typeof CURRENCIES)[number];
export const isCurrency = (value: unknown): value is Currency =>
  CURRENCIES.includes(value as Currency);

/**
 * Number options for an amount of money. Spend on LLMs is mostly
 * fractions of a cent, so small amounts keep their digits instead of
 * rounding to $0.00: up to four decimals below a dollar, three significant
 * digits below a cent. Formatting itself goes through use-intl, so the
 * reader's locale decides separators and symbol placement.
 */
export function moneyOptions(amount: number, currency: Currency = 'USD') {
  if (amount > 0 && amount < 0.01) {
    return {
      style: 'currency',
      currency,
      maximumSignificantDigits: 3,
    } as const;
  }
  return {
    style: 'currency',
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: amount > 0 && amount < 1 ? 4 : 2,
  } as const;
}

/** How much of a budget is spent, 0..1, or `null` when there is none. */
export function budgetShare(
  spent: number,
  budget: number | null,
): number | null {
  if (budget === null) return null;
  if (budget <= 0) return 1;
  return Math.min(spent / budget, 1);
}
