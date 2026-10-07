import { request } from '@/shared/api/http';
import { logLineSchema, pagedSchema } from '@/shared/api/schemas';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

export interface LogFilter extends ListParams {
  team_id?: number;
  key_id?: number;
  model?: string;
  end_user?: string;
  tag?: string;
  /** ISO timestamp: only requests since then. */
  from?: string;
  status?: 'success' | 'error';
}

export const getLogs = (filter: LogFilter) =>
  request(
    'GET',
    `/api/logs${toQuery({ ...filter })}`,
    pagedSchema(logLineSchema),
  );
