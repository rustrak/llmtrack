import { request } from '@/shared/api/http';
import {
  authorizationDecisionSchema,
  mcpSettingsSchema,
} from '@/shared/api/schemas';
import type { AuthorizationRequest } from './queries';

/** Opens or closes `/mcp`. Admins only; anyone else gets a 403. */
export const updateMcpSettings = (enabled: boolean) =>
  request('PUT', '/api/settings/mcp', mcpSettingsSchema, { enabled });

/** Answers a consent screen; the server says where to send the browser. */
export const decideAuthorization = (
  req: AuthorizationRequest,
  approve: boolean,
) =>
  request('POST', '/api/oauth/authorize', authorizationDecisionSchema, {
    ...req,
    approve,
  });

/** Disconnects an assistant: its tokens stop working at once. */
export const revokeGrant = (id: number) =>
  request('DELETE', `/api/oauth/grants/${id}`);
