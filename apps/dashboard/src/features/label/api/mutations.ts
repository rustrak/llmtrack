import { request } from '@/shared/api/http';
import { labelSchema } from '@/shared/api/schemas';
import type { LabelPayload } from '../model/label-form';

export const createLabel = (body: LabelPayload) =>
  request('POST', '/api/labels', labelSchema, body);

export const updateLabel = (id: number, body: LabelPayload) =>
  request('PATCH', `/api/labels/${id}`, labelSchema, body);

/** Its keys lose it; their spend stays. */
export const deleteLabel = (id: number) =>
  request('DELETE', `/api/labels/${id}`);
