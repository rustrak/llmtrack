export type RangePreset = '7d' | '30d' | '90d' | 'month' | 'last-month';

export const RANGE_PRESETS: readonly RangePreset[] = [
  '7d',
  '30d',
  '90d',
  'month',
  'last-month',
];

const DAY_MS = 24 * 60 * 60 * 1000;
const iso = (date: Date) => date.toISOString().slice(0, 10);

/** The UTC days a preset covers, as the server counts them. */
export function rangeFor(
  preset: RangePreset,
  today = new Date(),
): { from: string; to: string } {
  const y = today.getUTCFullYear();
  const m = today.getUTCMonth();
  switch (preset) {
    case 'month':
      return { from: iso(new Date(Date.UTC(y, m, 1))), to: iso(today) };
    case 'last-month':
      return {
        from: iso(new Date(Date.UTC(y, m - 1, 1))),
        to: iso(new Date(Date.UTC(y, m, 0))),
      };
    default: {
      const days = Number.parseInt(preset, 10);
      return {
        from: iso(new Date(today.getTime() - (days - 1) * DAY_MS)),
        to: iso(today),
      };
    }
  }
}

const DAY = /^\d{4}-\d{2}-\d{2}$/;

/** A custom `from`–`to` when both are days in order, else the preset (30 days by default). */
export function resolveRange(
  search: { range?: RangePreset; from?: string; to?: string },
  today = new Date(),
): { from: string; to: string } {
  const { from, to } = search;
  if (from && to && DAY.test(from) && DAY.test(to) && from <= to) {
    return { from, to };
  }
  return rangeFor(search.range ?? '30d', today);
}
