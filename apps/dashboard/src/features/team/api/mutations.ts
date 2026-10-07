import { request } from '@/shared/api/http';
import {
  memberSchema,
  personSchema,
  teamDetailSchema,
} from '@/shared/api/schemas';

export interface TeamPayload {
  name: string;
  max_budget_usd: number | null;
  budget_duration: string | null;
  rpm_limit: number | null;
  tpm_limit: number | null;
  max_parallel_requests: number | null;
  all_models: boolean;
  models: number[];
}

export const createTeam = (body: TeamPayload) =>
  request('POST', '/api/teams', teamDetailSchema, body);

export const updateTeam = (id: number, body: Partial<TeamPayload>) =>
  request('PATCH', `/api/teams/${id}`, teamDetailSchema, body);

export const deleteTeam = (id: number) => request('DELETE', `/api/teams/${id}`);

export const addMember = (teamId: number, email: string, role: string) =>
  request('POST', `/api/teams/${teamId}/members`, memberSchema, {
    email,
    role,
  });

export const updateMember = (teamId: number, userId: number, role: string) =>
  request('PATCH', `/api/teams/${teamId}/members/${userId}`, memberSchema, {
    role,
  });

export const removeMember = (teamId: number, userId: number) =>
  request('DELETE', `/api/teams/${teamId}/members/${userId}`);

export const createPerson = (body: {
  name: string;
  email: string | null;
  team_id: number | null;
}) => request('POST', '/api/people', personSchema, body);

/** `team_id` moves them; `null` takes them out of their team. */
export const updatePerson = (
  id: number,
  body: { name?: string; email?: string | null; team_id?: number | null },
) => request('PATCH', `/api/people/${id}`, personSchema, body);

export const deletePerson = (id: number) =>
  request('DELETE', `/api/people/${id}`);
