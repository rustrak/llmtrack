import { request } from '@/shared/api/http';
import { usageSchema } from '@/shared/api/schemas';
import { toQuery } from '@/shared/lib/list-params';
import type { GroupBy } from '../model/series';

export interface UsageFilter {
  from: string;
  to: string;
  team_id?: number;
  key_id?: number;
  model?: string;
  person_id?: number;
  group_by?: GroupBy;
}

export const getUsage = (filter: UsageFilter) =>
  request('GET', `/api/usage${toQuery({ ...filter })}`, usageSchema);
