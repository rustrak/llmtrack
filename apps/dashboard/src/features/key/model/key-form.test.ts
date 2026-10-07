import { describe, expect, it } from 'vitest';
import {
  type CreateKeyValues,
  createKeySchema,
  editDefaults,
  snippets,
  toCreateKeyPayload,
  toUpdateKeyPayload,
} from './key-form';

const now = new Date('2026-10-07T12:00:00Z');
const base: CreateKeyValues = {
  team_id: '3',
  person_id: '',
  name: '  backend ',
  models: [2, 1],
  max_budget_usd: '25.5',
  budget_duration: 'none',
  rpm_limit: '',
  tpm_limit: '',
  max_parallel_requests: '',
  expires: '30',
};

describe('toCreateKeyPayload', () => {
  it('turns form strings into the API shape', () => {
    expect(toCreateKeyPayload(base, now)).toEqual({
      team_id: 3,
      name: 'backend',
      models: [2, 1],
      max_budget_usd: 25.5,
      expires_at: '2026-11-06T12:00:00.000Z',
    });
  });

  it('assigns a team key to one of its people, never a personal key', () => {
    expect(toCreateKeyPayload({ ...base, person_id: '9' }, now).person_id).toBe(
      9,
    );
    expect(
      toCreateKeyPayload({ ...base, team_id: 'personal', person_id: '9' }, now),
    ).not.toHaveProperty('person_id');
  });

  it('leaves out what is empty, and a personal key has no team', () => {
    const payload = toCreateKeyPayload(
      { ...base, team_id: 'personal', max_budget_usd: '', expires: 'never' },
      now,
    );
    expect(payload).toEqual({ name: 'backend', models: [2, 1] });
  });

  it('sends the budget period and limits when set', () => {
    expect(
      toCreateKeyPayload(
        {
          ...base,
          budget_duration: '30d',
          rpm_limit: '60',
          tpm_limit: '1000',
          max_parallel_requests: '7',
        },
        now,
      ),
    ).toMatchObject({
      budget_duration: '30d',
      rpm_limit: 60,
      tpm_limit: 1000,
      max_parallel_requests: 7,
    });
  });
});

describe('editing', () => {
  const key = {
    name: 'k',
    models: [{ id: 1, name: 'gpt' }],
    max_budget_usd: null,
    budget_duration: '7d',
    rpm_limit: 10,
    tpm_limit: null,
    max_parallel_requests: 4,
    expires_at: '2027-01-15T22:59:59.000Z',
    person_id: 9,
  };

  it('starts from the key as it is', () => {
    expect(editDefaults(key)).toEqual({
      name: 'k',
      models: [1],
      person_id: '9',
      max_budget_usd: '',
      budget_duration: '7d',
      rpm_limit: '10',
      tpm_limit: '',
      max_parallel_requests: '4',
      expires_on: expect.stringMatching(/^2027-01-1[56]$/),
    });
  });

  it('clears with explicit nulls and leaves an untouched expiry out', () => {
    const values = editDefaults(key);
    expect(
      toUpdateKeyPayload(
        {
          ...values,
          person_id: '',
          budget_duration: 'none',
          rpm_limit: '',
          max_parallel_requests: '',
        },
        values.expires_on,
      ),
    ).toEqual({
      name: 'k',
      models: [1],
      person_id: null,
      max_budget_usd: null,
      budget_duration: null,
      rpm_limit: null,
      tpm_limit: null,
      max_parallel_requests: null,
    });
  });

  it('shows and saves the budget in the reader’s currency', () => {
    const values = editDefaults({ ...key, max_budget_usd: 10 / 0.86 }, 0.86);
    expect(values.max_budget_usd).toBe('10');
    expect(
      toUpdateKeyPayload(values, values.expires_on, 0.86).max_budget_usd,
    ).toBeCloseTo(10 / 0.86);
  });

  it('sends a new expiry as the end of that day, or null for never', () => {
    const values = editDefaults(key);
    const later = toUpdateKeyPayload(
      { ...values, expires_on: '2027-02-01' },
      values.expires_on,
    );
    expect(new Date(later.expires_at as string).getDate()).toBe(1);
    expect(
      toUpdateKeyPayload({ ...values, expires_on: '' }, values.expires_on)
        .expires_at,
    ).toBeNull();
  });
});

describe('snippets', () => {
  it('point the OpenAI SDK at the gateway with the new key', () => {
    const s = snippets('https://llm.acme.dev', 'sk-abc', 'gpt-4o');
    expect(s.curl).toContain('https://llm.acme.dev/v1/chat/completions');
    expect(s.curl).toContain('Bearer sk-abc');
    expect(s.python).toContain('base_url="https://llm.acme.dev/v1"');
    expect(s.python).toContain('model="gpt-4o"');
    expect(s.node).toContain("apiKey: 'sk-abc'");
    expect(s.anthropic).toContain('ANTHROPIC_BASE_URL="https://llm.acme.dev"');
  });
});

describe('createKeySchema', () => {
  const schema = createKeySchema((key) => `t:${key}`);

  it('words its errors through the translator', () => {
    const result = schema.safeParse({ ...base, name: ' ' });
    expect(result.success).toBe(false);
    expect(result.error?.issues[0].message).toBe('t:form.nameRequired');
  });

  it('takes whole positive limits only', () => {
    expect(schema.safeParse({ ...base, rpm_limit: '0' }).success).toBe(false);
    expect(schema.safeParse({ ...base, tpm_limit: '1.5' }).success).toBe(false);
    expect(schema.safeParse({ ...base, rpm_limit: '60' }).success).toBe(true);
  });
});
