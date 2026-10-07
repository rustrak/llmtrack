import { request } from '@/shared/api/http';
import { keySchema, pagedSchema } from '@/shared/api/schemas';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

export type KeyStatusFilter = 'active' | 'blocked' | 'expired';

export interface KeyListParams extends ListParams {
  team_id?: number;
  status?: KeyStatusFilter;
}

export const listKeys = (params: KeyListParams = {}) =>
  request('GET', `/api/keys${toQuery({ ...params })}`, pagedSchema(keySchema));

export const getKey = (id: number) =>
  request('GET', `/api/keys/${id}`, keySchema);
