import { request } from '@/shared/api/http';
import { createdKeySchema, keySchema } from '@/shared/api/schemas';
import type { CreateKeyPayload, toUpdateKeyPayload } from '../model/key-form';

export const createKey = (body: CreateKeyPayload) =>
  request('POST', '/api/keys', createdKeySchema, body);

export const updateKey = (
  id: number,
  body: ReturnType<typeof toUpdateKeyPayload>,
) => request('PATCH', `/api/keys/${id}`, keySchema, body);

export const setKeyBlocked = (id: number, blocked: boolean) =>
  request(
    'POST',
    `/api/keys/${id}/${blocked ? 'block' : 'unblock'}`,
    keySchema,
  );

export const regenerateKey = (id: number) =>
  request('POST', `/api/keys/${id}/regenerate`, createdKeySchema);

export const revokeKey = (id: number) => request('DELETE', `/api/keys/${id}`);
