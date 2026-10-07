import { describe, expect, it } from 'vitest';
import type { Usage } from '@/shared/api/schemas';
import { usageCsv } from './csv';

const totals = {
  cost_usd: 1.5,
  requests: 3,
  failed_requests: 1,
  prompt_tokens: 100,
  completion_tokens: 50,
  cached_tokens: 10,
  cache_write_tokens: 0,
  reasoning_tokens: 5,
  latency_ms: 900,
};

const usage: Usage = {
  from: '2026-10-01',
  to: '2026-10-02',
  totals,
  previous: totals,
  series: [],
  daily: [{ day: '2026-10-01', ...totals }],
  by_team: [{ team_id: null, team_name: null, ...totals }],
  by_key: [
    {
      key_id: 4,
      key_name: 'backend, "prod"',
      key_hint: 'sk-...abcd',
      team_name: 'Acme',
      ...totals,
    },
  ],
  by_person: [
    { person_id: 9, person_name: 'Lucía', team_name: 'Acme', ...totals },
  ],
  by_model: [{ model_name: 'gpt-4o', ...totals }],
  by_end_user: [{ end_user: 'customer-1', ...totals }],
  by_tag: [{ tag: 'prod', ...totals }],
};

describe('usageCsv', () => {
  it('lists every breakdown with raw numbers, quoting what needs it', () => {
    const lines = usageCsv(usage, 'Personal').split('\n');
    expect(lines[0]).toBe(
      'group,name,requests,failed_requests,prompt_tokens,completion_tokens,cached_tokens,cache_write_tokens,reasoning_tokens,cost_usd',
    );
    expect(lines).toContain('day,2026-10-01,3,1,100,50,10,0,5,1.5');
    expect(lines).toContain('team,Personal,3,1,100,50,10,0,5,1.5');
    expect(lines).toContain('key,"backend, ""prod""",3,1,100,50,10,0,5,1.5');
    expect(lines).toContain('person,Lucía,3,1,100,50,10,0,5,1.5');
    expect(lines).toContain('model,gpt-4o,3,1,100,50,10,0,5,1.5');
    expect(lines).toContain('customer,customer-1,3,1,100,50,10,0,5,1.5');
    expect(lines).toContain('tag,prod,3,1,100,50,10,0,5,1.5');
  });
});
