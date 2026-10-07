import { z } from 'zod';
import { request } from '@/shared/api/http';
import {
  invitationInfoSchema,
  invitationSchema,
  pagedSchema,
  userListItemSchema,
} from '@/shared/api/schemas';
import { type ListParams, toQuery } from '@/shared/lib/list-params';

export interface UserListParams extends ListParams {
  role?: 'admin' | 'member';
  status?: 'active' | 'inactive';
}

export const listUsers = (params: UserListParams = {}) =>
  request(
    'GET',
    `/api/users${toQuery({ ...params })}`,
    pagedSchema(userListItemSchema),
  );

export const listInvitations = () =>
  request('GET', '/api/invitations', z.array(invitationSchema));

export const getInvitation = (token: string) =>
  request(
    'GET',
    `/auth/invitation/${encodeURIComponent(token)}`,
    invitationInfoSchema,
  );
