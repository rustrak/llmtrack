import { describe, expect, it } from 'vitest';
import { labelDefaults, labelSchema, toLabelPayload } from './label-form';

const t = (key: string) => `t:${key}`;

describe('label form', () => {
  it('starts grey and empty, or from the label', () => {
    expect(labelDefaults()).toEqual({
      name: '',
      color: 'gray',
      description: '',
    });
    expect(
      labelDefaults({
        id: 1,
        name: 'CRM',
        color: 'blue',
        description: null,
        created_at: '2026-10-08T00:00:00Z',
        key_count: 2,
      }),
    ).toEqual({ name: 'CRM', color: 'blue', description: '' });
  });

  it('trims and sends an empty description as none', () => {
    expect(
      toLabelPayload({ name: ' CRM ', color: 'blue', description: '  ' }),
    ).toEqual({ name: 'CRM', color: 'blue', description: null });
  });

  it('is bounded like the server', () => {
    const schema = labelSchema(t);
    const ok = { name: 'CRM', color: 'blue', description: '' };
    expect(schema.safeParse(ok).success).toBe(true);
    expect(
      schema.safeParse({ ...ok, name: ' ' }).error?.issues[0].message,
    ).toBe('t:form.nameRequired');
    expect(
      schema.safeParse({ ...ok, name: 'x'.repeat(51) }).error?.issues[0]
        .message,
    ).toBe('t:form.nameTooLong');
    expect(schema.safeParse({ ...ok, color: 'chartreuse' }).success).toBe(
      false,
    );
  });
});
