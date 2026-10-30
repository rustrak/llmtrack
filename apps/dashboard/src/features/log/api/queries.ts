import { request } from '@/shared/api/http';
import { bodySchema, logLineSchema, pagedSchema } from '@/shared/api/schemas';
import type { FilterParams } from '@/shared/lib/filters';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

/** The list filters of `GET /api/logs`, each with its `exclude_` twin. */
export const LOG_FILTERS = [
  'team_id',
  'key_id',
  'model',
  'label_id',
  'end_user',
  'tag',
] as const;

export interface LogFilter
  extends ListParams,
    FilterParams<(typeof LOG_FILTERS)[number]> {
  /** ISO timestamp: only requests since then. */
  from?: string;
  status?: 'success' | 'error';
  /** Every word in the stored request or response bodies. */
  text?: string;
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
