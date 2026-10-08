import { toQuery } from '@/shared/lib/list-params';

/** What a report can hold, in the order it holds them (the server's `SECTIONS`). */
export const REPORT_SECTIONS = [
  'summary',
  'trend',
  'tokens',
  'teams',
  'keys',
  'people',
  'models',
  'customers',
  'tags',
  'daily',
  'lines',
  'requests',
  'rates',
] as const;
export type ReportSection = (typeof REPORT_SECTIONS)[number];

/** One row per request is long: asked for, never assumed. */
export const DEFAULT_SECTIONS: ReportSection[] = REPORT_SECTIONS.filter(
  (s) => s !== 'requests',
);

export type ReportFormat = 'pdf' | 'xlsx';

export interface ReportOptions {
  format: ReportFormat;
  sections: ReportSection[];
  /** Percent over provider cost, as typed; blank is none. */
  markup: string;
  showCost: boolean;
  client: string;
  reference: string;
  notes: string;
}

export interface ReportScope {
  from: string;
  to: string;
  team_id?: number;
  key_id?: number;
  model?: string;
  person_id?: number;
  label_id?: number;
}

export const MAX_MARKUP = 1000;

/** The markup as a number, `null` when it is not one the server takes. */
export function parseMarkup(markup: string): number | null {
  const text = markup.trim().replace(',', '.');
  if (text === '') return 0;
  const value = Number(text);
  return Number.isFinite(value) && value >= 0 && value <= MAX_MARKUP
    ? value
    : null;
}

/** The export URL for the usage page's filters and the dialog's choices. */
export function reportUrl(scope: ReportScope, options: ReportOptions): string {
  const markup = parseMarkup(options.markup) ?? 0;
  const sections = REPORT_SECTIONS.filter((s) => options.sections.includes(s));
  return `/api/usage/export${toQuery({
    format: options.format,
    from: scope.from,
    to: scope.to,
    team_id: scope.team_id,
    key_id: scope.key_id,
    model: scope.model,
    person_id: scope.person_id,
    label_id: scope.label_id,
    // The server's default is every section but the requests.
    sections:
      sections.join(',') === DEFAULT_SECTIONS.join(',')
        ? undefined
        : sections.join(','),
    markup: markup > 0 ? markup : undefined,
    // Cost beside the amount only means something with a markup.
    show_cost: markup > 0 && options.showCost ? 'true' : undefined,
    client: options.client.trim(),
    reference: options.reference.trim(),
    notes: options.notes.trim(),
  })}`;
}

/** The file name the server sends, from its Content-Disposition. */
export function fileNameFrom(disposition: string | null, fallback: string) {
  return disposition?.match(/filename="([^"]+)"/)?.[1] ?? fallback;
}

/** How the dialog groups the sections, in report order. */
export const SECTION_GROUPS = [
  { key: 'overview', sections: ['summary', 'trend', 'tokens'] },
  {
    key: 'breakdowns',
    sections: ['teams', 'keys', 'people', 'models', 'customers', 'tags'],
  },
  { key: 'detail', sections: ['daily', 'lines', 'requests'] },
  { key: 'annex', sections: ['rates'] },
] as const satisfies readonly {
  key: string;
  sections: readonly ReportSection[];
}[];

/**
 * Breakdowns the filters already answer: filtered to one team, a table by
 * team has one row. The server leaves them out too.
 */
export function narrowedBy(scope: ReportScope): ReportSection[] {
  const out: ReportSection[] = [];
  if (scope.team_id) out.push('teams');
  if (scope.key_id) out.push('keys');
  if (scope.person_id) out.push('people');
  if (scope.model) out.push('models');
  return out;
}
