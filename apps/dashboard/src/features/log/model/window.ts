/** How far back the log looks; labelled in the page. */
export const LOG_WINDOWS = ['1h', '24h', '7d', '30d', 'all'] as const;
export type LogWindow = (typeof LOG_WINDOWS)[number];

const HOUR_MS = 60 * 60 * 1000;
const SPANS: Record<Exclude<LogWindow, 'all'>, number> = {
  '1h': HOUR_MS,
  '24h': 24 * HOUR_MS,
  '7d': 7 * 24 * HOUR_MS,
  '30d': 30 * 24 * HOUR_MS,
};

export function parseWindow(value: unknown): LogWindow {
  return LOG_WINDOWS.find((w) => w === value) ?? '24h';
}

/** The `from` timestamp a window asks the server for; none for `all`. */
export function since(window: LogWindow, now = new Date()) {
  return window === 'all'
    ? undefined
    : new Date(now.getTime() - SPANS[window]).toISOString();
}
