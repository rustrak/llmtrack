import { describe, expect, it } from 'vitest';
import { teamDefaults, toTeamPayload } from './team-form';

const values = {
  name: ' Acme ',
  max_budget_usd: '100',
  budget_duration: '30d',
  rpm_limit: '',
  tpm_limit: '5000',
  max_parallel_requests: '7',
  all_models: false,
  models: [3],
};

describe('toTeamPayload', () => {
  it('parses numbers, trims the name and clears empty fields with null', () => {
    expect(toTeamPayload(values)).toEqual({
      name: 'Acme',
      max_budget_usd: 100,
      budget_duration: '30d',
      rpm_limit: null,
      tpm_limit: 5000,
      max_parallel_requests: 7,
      all_models: false,
      models: [3],
    });
  });

  it('converts a budget typed in euros to the dollars it is stored in', () => {
    expect(toTeamPayload(values, 0.8).max_budget_usd).toBe(125);
    expect(
      teamDefaults(
        {
          ...values,
          max_budget_usd: 125,
          budget_duration: null,
          rpm_limit: null,
          tpm_limit: null,
          max_parallel_requests: null,
          models: [],
        },
        0.8,
      ).max_budget_usd,
    ).toBe('100');
  });

  it('sends no list when the team gets every model', () => {
    expect(toTeamPayload({ ...values, all_models: true }).models).toEqual([]);
  });

  it('starts a new team with every model and no period', () => {
    expect(teamDefaults()).toMatchObject({
      all_models: true,
      budget_duration: 'none',
    });
  });
});
