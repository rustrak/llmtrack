import { request } from '@/shared/api/http';
import {
  pagedSchema,
  personSchema,
  teamDetailSchema,
  teamSchema,
} from '@/shared/api/schemas';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

export interface TeamListParams extends ListParams {
  models?: 'all' | 'restricted';
}

export const listTeams = (params: TeamListParams = {}) =>
  request(
    'GET',
    `/api/teams${toQuery({ ...params })}`,
    pagedSchema(teamSchema),
  );

export const getTeam = (id: number) =>
  request('GET', `/api/teams/${id}`, teamDetailSchema);

export interface PeopleListParams extends ListParams {
  team_id?: number;
  /** Only people without a team (admins only see them). */
  unassigned?: boolean;
}

/** Every visible team's people. */
export const listPeople = (params: PeopleListParams = {}) =>
  request(
    'GET',
    `/api/people${toQuery({ ...params, unassigned: params.unassigned ? 'true' : undefined })}`,
    pagedSchema(personSchema),
  );
