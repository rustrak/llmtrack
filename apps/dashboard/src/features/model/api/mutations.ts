import { request } from '@/shared/api/http';
import {
  catalogStatusSchema,
  modelSchema,
  probeSchema,
  type RouterSettings,
  routerSettingsSchema,
} from '@/shared/api/schemas';
import type { ModelPayload } from '../model/model-form';

export const createModel = (body: ModelPayload) =>
  request('POST', '/api/models', modelSchema, body);

export const updateModel = (
  id: number,
  body: Partial<ModelPayload> & { is_active?: boolean },
) => request('PATCH', `/api/models/${id}`, modelSchema, body);

export const deleteModel = (id: number) =>
  request('DELETE', `/api/models/${id}`);

/** Downloads the latest price list and reprices catalog models. */
export const syncCatalog = () =>
  request('POST', '/api/catalog/sync', catalogStatusSchema);

/** "Test Connect": one token from the provider with the settings as typed. */
export const testModel = (body: {
  model_id?: number;
  provider: string;
  upstream_model: string;
  api_base: string | null;
  api_version: string | null;
  api_key?: string | null;
}) => request('POST', '/api/models/test', probeSchema, body);

export const saveRouterSettings = (body: RouterSettings) =>
  request('PUT', '/api/router-settings', routerSettingsSchema, body);
