import { request } from '@/shared/api/http';
import { bodySchema, logLineSchema, pagedSchema } from '@/shared/api/schemas';
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

export const getBody = (requestId: string) =>
  request('GET', `/api/logs/${encodeURIComponent(requestId)}/body`, bodySchema);

/** `json`: every field and both bodies; `chat`: OpenAI's fine-tuning format. */
export type ExportFormat = 'json' | 'chat';

/** The stored bodies behind these logs, as JSON Lines to download. */
export const exportUrl = (filter: LogFilter, format: ExportFormat) =>
  `/api/logs/export${toQuery({ ...filter, page: undefined, per_page: undefined, sort: undefined, format })}`;
