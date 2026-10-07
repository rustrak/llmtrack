import { describe, expect, it } from 'vitest';
import type { ApiError } from '@/shared/api/http';
import { serverFieldErrors } from './form-errors';

const t = (key: string) => `[${key}]`;
const error = (fields: ApiError['fields'], message = 'nope'): ApiError => ({
  kind: 'conflict',
  status: 409,
  message,
  fields,
});

describe('serverFieldErrors', () => {
  it('pins known fields, including nested paths, with translated copy', () => {
    expect(
      serverFieldErrors(
        error([
          { field: 'name', code: 'already_exists' },
          { field: 'pricing.input', code: 'invalid' },
        ]),
        ['name', 'pricing', 'pricing.input'],
        t,
      ),
    ).toEqual({
      fields: { name: '[alreadyExists]', 'pricing.input': '[invalid]' },
      root: null,
    });
  });

  it('sends unknown fields and field-less errors to the form root', () => {
    expect(
      serverFieldErrors(
        error([{ field: 'models', code: 'invalid' }], 'model 9 is not allowed'),
        ['name'],
        t,
      ),
    ).toEqual({ fields: {}, root: 'model 9 is not allowed' });
    expect(serverFieldErrors(error([], 'boom'), ['name'], t)).toEqual({
      fields: {},
      root: 'boom',
    });
  });

  it('strips the error-type prefix the server puts before the detail', () => {
    expect(
      serverFieldErrors(error([], 'Conflict: team exists'), [], t).root,
    ).toBe('team exists');
  });
});
