import { createFormatter } from 'use-intl';
import { describe, expect, it } from 'vitest';
import { budgetShare, moneyOptions } from './format';

const format = (locale: string) => createFormatter({ locale, timeZone: 'UTC' });
const usd = (amount: number, locale = 'en') =>
  format(locale).number(amount, moneyOptions(amount));

describe('moneyOptions', () => {
  it('shows cents for ordinary amounts', () => {
    expect(usd(0)).toBe('$0.00');
    expect(usd(1234.5)).toBe('$1,234.50');
  });

  it('keeps up to four decimals below a dollar', () => {
    expect(usd(0.0125)).toBe('$0.0125');
    expect(usd(0.5)).toBe('$0.50');
  });

  it('keeps three significant digits below a cent', () => {
    expect(usd(0.000225)).toBe('$0.000225');
  });

  it('follows the reader’s locale', () => {
    expect(usd(1234.5, 'es')).toBe('1234,50\u00a0US$');
  });

  it('shows the reader’s currency', () => {
    expect(format('es').number(0.5, moneyOptions(0.5, 'EUR'))).toBe(
      '0,50\u00a0€',
    );
  });
});

describe('budgetShare', () => {
  it('is the spent fraction, capped at 1, or null without a budget', () => {
    expect(budgetShare(5, 10)).toBe(0.5);
    expect(budgetShare(15, 10)).toBe(1);
    expect(budgetShare(5, null)).toBeNull();
    expect(budgetShare(0, 0)).toBe(1);
  });
});
