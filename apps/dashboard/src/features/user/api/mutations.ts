import { z } from 'zod';
import { request } from '@/shared/api/http';
import { invitationSchema, userSchema } from '@/shared/api/schemas';
import { session } from '@/shared/api/session';

export async function login(email: string, password: string) {
  const result = await request(
    'POST',
    '/auth/login',
    z.object({ user: userSchema }),
    { email, password },
  );
  if (result.success) {
    session.set({ state: 'authenticated', user: result.data.user });
  }
  return result;
}

export async function logout() {
  const result = await request('POST', '/auth/logout');
  session.clear();
  return result;
}

export const changePassword = (
  current_password: string,
  new_password: string,
) =>
  request('POST', '/auth/me/password', undefined, {
    current_password,
    new_password,
  });

/** Edits the signed-in user's profile; the session learns the new values. */
export async function updateProfile(changes: {
  name?: string | null;
  language?: string | null;
  timezone?: string | null;
  currency?: string | null;
  currency_rate?: number | null;
}) {
  const result = await request(
    'PATCH',
    '/auth/me',
    z.object({ user: userSchema }),
    changes,
  );
  if (result.success) {
    session.set({ state: 'authenticated', user: result.data.user });
  }
  return result;
}

export const createUser = (body: {
  email: string;
  password: string;
  role: string;
}) => request('POST', '/api/users', userSchema, body);

export const updateUser = (
  id: number,
  body: { role?: string; is_active?: boolean; password?: string },
) => request('PATCH', `/api/users/${id}`, userSchema, body);

export const deleteUser = (id: number) => request('DELETE', `/api/users/${id}`);

export const createInvitation = (body: { email: string; role: string }) =>
  request('POST', '/api/invitations', invitationSchema, body);

export const revokeInvitation = (token: string) =>
  request('DELETE', `/api/invitations/${encodeURIComponent(token)}`);

/** Sets the invitee's password; the server signs them in. */
export async function acceptInvitation(token: string, password: string) {
  const result = await request(
    'POST',
    '/auth/accept-invitation',
    z.object({ user: userSchema }),
    { token, password },
  );
  if (result.success) {
    session.set({ state: 'authenticated', user: result.data.user });
  }
  return result;
}
