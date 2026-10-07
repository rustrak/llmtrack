import { request } from '@/shared/api/http';
import { settingsSchema } from '@/shared/api/schemas';

export const getSettings = () =>
  request('GET', '/api/settings', settingsSchema);
