import { describe, expect, it, vi } from 'vitest';
import { type CurrentUser, createSessionStore } from './session';

const user = {
  id: 1,
  email: 'a@b.co',
  role: 'admin' as const,
  is_active: true,
  created_at: '2026-01-01T00:00:00Z',
  last_login: null,
};

describe('session store', () => {
  it('asks the server once while a request is in flight', async () => {
    const read = vi.fn(
      async (): Promise<CurrentUser> => ({ state: 'authenticated', user }),
    );
    const store = createSessionStore(read);
    await Promise.all([store.ensure(), store.ensure()]);
    await store.ensure();
    expect(read).toHaveBeenCalledTimes(1);
  });

  it('retries after the server was unavailable', async () => {
    const read = vi
      .fn<() => Promise<CurrentUser>>()
      .mockResolvedValueOnce({
        state: 'unavailable',
        error: { kind: 'network', status: 0, message: 'down', fields: [] },
      })
      .mockResolvedValueOnce({ state: 'anonymous' });
    const store = createSessionStore(read);
    expect((await store.ensure()).state).toBe('unavailable');
    expect((await store.ensure()).state).toBe('anonymous');
  });

  it('set and clear replace the answer without asking', async () => {
    const read = vi.fn(
      async (): Promise<CurrentUser> => ({ state: 'anonymous' }),
    );
    const store = createSessionStore(read);
    store.set({ state: 'authenticated', user });
    expect((await store.ensure()).state).toBe('authenticated');
    store.clear();
    expect(store.peek()).toEqual({ state: 'anonymous' });
    expect(read).not.toHaveBeenCalled();
  });
});
