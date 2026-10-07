import { request } from '@/shared/api/http';
import { settingsSchema } from '@/shared/api/schemas';

export const updateSettings = (changes: {
  public_url?: string | null;
  price_catalog_url?: string | null;
}) => request('PATCH', '/api/settings', settingsSchema, changes);
