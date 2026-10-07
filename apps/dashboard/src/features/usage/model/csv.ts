import type { Totals, Usage } from '@/shared/api/schemas';

const COLUMNS = [
  'requests',
  'failed_requests',
  'prompt_tokens',
  'completion_tokens',
  'cached_tokens',
  'cache_write_tokens',
  'reasoning_tokens',
  'cost_usd',
] as const satisfies readonly (keyof Totals)[];

const cell = (value: string) =>
  /[",\n]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;

/** The report as CSV: one row per day, team, key, person, model, customer and tag. */
export function usageCsv(usage: Usage, personal: string) {
  const row = (group: string, name: string, totals: Totals) =>
    [group, cell(name), ...COLUMNS.map((c) => String(totals[c]))].join(',');
  return [
    ['group', 'name', ...COLUMNS].join(','),
    ...usage.daily.map((d) => row('day', d.day, d)),
    ...usage.by_team.map((d) => row('team', d.team_name ?? personal, d)),
    ...usage.by_key.map((d) => row('key', d.key_name, d)),
    ...usage.by_person.map((d) => row('person', d.person_name, d)),
    ...usage.by_model.map((d) => row('model', d.model_name, d)),
    ...usage.by_end_user.map((d) => row('customer', d.end_user, d)),
    ...usage.by_tag.map((d) => row('tag', d.tag, d)),
  ].join('\n');
}
