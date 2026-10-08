import { errorFrom, type Result, request } from '@/shared/api/http';
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
  /** Only the keys carrying this label now. */
  label_id?: number;
  group_by?: GroupBy;
}

export const getUsage = (filter: UsageFilter) =>
  request('GET', `/api/usage${toQuery({ ...filter })}`, usageSchema);

/**
 * A rendered report, fetched as a file so that a refusal comes back as an
 * error to show rather than as a JSON file in the downloads folder.
 */
export async function fetchReport(
  url: string,
): Promise<Result<{ blob: Blob; disposition: string | null }>> {
  let response: Response;
  try {
    response = await fetch(url, { credentials: 'same-origin' });
  } catch (cause) {
    return {
      success: false,
      error: {
        kind: 'network',
        status: 0,
        message: cause instanceof Error ? cause.message : String(cause),
        fields: [],
      },
    };
  }
  if (!response.ok) {
    return { success: false, error: await errorFrom(response) };
  }
  return {
    success: true,
    data: {
      blob: await response.blob(),
      disposition: response.headers.get('content-disposition'),
    },
  };
}
