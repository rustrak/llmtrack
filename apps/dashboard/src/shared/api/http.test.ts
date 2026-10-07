import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { request } from './http';

function respond(status: number, body?: unknown) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async () =>
      body === undefined
        ? new Response(null, { status })
        : new Response(JSON.stringify(body), {
            status,
            headers: { 'content-type': 'application/json' },
          }),
    ),
  );
}

const thing = z.object({ id: z.number() });

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('request', () => {
  it('parses a successful body with the schema', async () => {
    respond(200, { id: 7, extra: true });
    const result = await request('GET', '/api/things/7', thing);
    expect(result).toEqual({ success: true, data: { id: 7 } });
  });

  it('sends JSON with the session cookie', async () => {
    respond(201, { id: 1 });
    await request('POST', '/api/things', thing, { name: 'a' });
    const [url, init] = vi.mocked(fetch).mock.calls[0];
    expect(url).toBe('/api/things');
    expect(init?.method).toBe('POST');
    expect(init?.credentials).toBe('same-origin');
    expect(init?.body).toBe('{"name":"a"}');
  });

  it('answers 204 with undefined', async () => {
    respond(204);
    expect(await request('DELETE', '/api/things/1')).toEqual({
      success: true,
      data: undefined,
    });
  });

  it.each([
    [401, 'unauthenticated'],
    [403, 'forbidden'],
    [404, 'not_found'],
    [409, 'conflict'],
    [400, 'validation'],
    [429, 'rate_limited'],
    [502, 'server'],
  ] as const)('maps %i to %s', async (status, kind) => {
    respond(status, { error: { type: 'X', message: `boom ${status}` } });
    const result = await request('GET', '/api/x', thing);
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.kind).toBe(kind);
      expect(result.error.message).toBe(`boom ${status}`);
    }
  });

  it('keeps per-field errors', async () => {
    respond(409, {
      error: {
        type: 'Conflict',
        message: 'taken',
        fields: [{ field: 'name', code: 'already_exists' }],
      },
    });
    const result = await request('POST', '/api/x', thing, {});
    expect(!result.success && result.error.fields).toEqual([
      { field: 'name', code: 'already_exists' },
    ]);
  });

  it('reports a body that does not match the schema', async () => {
    respond(200, { id: 'seven' });
    const result = await request('GET', '/api/x', thing);
    expect(!result.success && result.error.kind).toBe('invalid_response');
  });

  it('reports a network failure', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new TypeError('Failed to fetch');
      }),
    );
    const result = await request('GET', '/api/x', thing);
    expect(!result.success && result.error.kind).toBe('network');
  });
});
