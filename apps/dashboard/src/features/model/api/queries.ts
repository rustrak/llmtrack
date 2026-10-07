import { z } from 'zod';
import { request } from '@/shared/api/http';
import {
  catalogEntrySchema,
  catalogStatusSchema,
  modelSchema,
  pagedSchema,
  providerSchema,
  routerSettingsSchema,
} from '@/shared/api/schemas';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

export interface ModelListParams extends ListParams {
  provider?: string;
  status?: 'active' | 'inactive';
}

export const listModels = (params: ModelListParams = {}) =>
  request(
    'GET',
    `/api/models${toQuery({ ...params })}`,
    pagedSchema(modelSchema.extend({ deployments: z.number() })),
  );

export const listProviders = () =>
  request('GET', '/api/providers', z.array(providerSchema));

/** The models the price list knows for one provider. */
export const listProviderModels = (provider: string) =>
  request(
    'GET',
    `/api/catalog?${new URLSearchParams({ provider, limit: '1000' })}`,
    z.array(catalogEntrySchema),
  );

export const getCatalogStatus = () =>
  request('GET', '/api/catalog/status', catalogStatusSchema);

/** Admins only: anyone else gets a 403, which the page reads as "hide it". */
export const getRouterSettings = () =>
  request('GET', '/api/router-settings', routerSettingsSchema);

/** Any entry of the price list, by the words of its key. */
export const searchCatalog = (q: string) =>
  request(
    'GET',
    `/api/catalog?${new URLSearchParams({ q, limit: '8' })}`,
    z.array(catalogEntrySchema),
  );
