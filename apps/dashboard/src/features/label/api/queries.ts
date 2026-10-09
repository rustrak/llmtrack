import { request } from '@/shared/api/http';
import { labelSchema, pagedSchema } from '@/shared/api/schemas';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

export const listLabels = (params: ListParams = {}) =>
  request(
    'GET',
    `/api/labels${toQuery({ ...params })}`,
    pagedSchema(labelSchema),
  );
