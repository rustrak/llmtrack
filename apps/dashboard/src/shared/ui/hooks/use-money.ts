import { useSyncExternalStore } from 'react';
import { useFormatter } from 'use-intl';
import { intl } from '@/shared/i18n/intl';
import { moneyOptions } from '@/shared/lib/format';

/** Locale-aware formatters for the numbers this dashboard shows most. */
export function useMoney() {
  const format = useFormatter();
  const { currency, perUsd } = useSyncExternalStore(
    intl.subscribe,
    intl.snapshot,
    intl.snapshot,
  );
  /** An amount already in the reader's currency, as typed in a form. */
  const local = (amount: number) =>
    format.number(amount, moneyOptions(amount, currency));
  return {
    /** Dollars from the server, shown in the reader's currency. */
    usd: (amount: number) => local(amount * perUsd),
    local,
    currency,
    perUsd,
    compact: (value: number) => format.number(value, 'compact'),
    dateTime: (iso: string) => format.dateTime(new Date(iso), 'dateTime'),
    date: (iso: string) => format.dateTime(new Date(iso), 'date'),
  };
}
