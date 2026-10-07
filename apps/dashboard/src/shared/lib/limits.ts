import { z } from 'zod';

/**
 * Budgets and rate limits, as keys and teams both take them. Error messages
 * are `form.budgetInvalid` and `form.limitInvalid` in the caller's namespace.
 */
export type Translate = (key: string) => string;

/** How often a budget starts over; labelled in the dialog. */
export const BUDGET_PERIODS = ['none', '1d', '7d', '30d'] as const;

/** Empty, or a non-negative dollar amount. */
export function budgetField(t: Translate) {
  return z
    .string()
    .trim()
    .refine((v) => v === '' || (Number.isFinite(Number(v)) && Number(v) >= 0), {
      message: t('form.budgetInvalid'),
    });
}

/** Empty, or a whole number of at least one. */
export function limitField(t: Translate) {
  return z
    .string()
    .trim()
    .refine(
      (v) => v === '' || (Number.isInteger(Number(v)) && Number(v) >= 1),
      {
        message: t('form.limitInvalid'),
      },
    );
}

export const numberOrNull = (value: string) =>
  value.trim() === '' ? null : Number(value);

/** A budget typed in the reader's currency, as the USD it is stored in. */
export const budgetOrNull = (value: string, perUsd = 1) => {
  const amount = numberOrNull(value);
  return amount === null ? null : amount / perUsd;
};

/**
 * A stored USD budget as the reader types it. Six decimals hide the float
 * left over from the round trip through another currency.
 */
export const budgetText = (usd: number | null | undefined, perUsd = 1) =>
  usd == null ? '' : String(Number((usd * perUsd).toFixed(6)));

export const periodOrNull = (value: string) =>
  value === 'none' ? null : value;
