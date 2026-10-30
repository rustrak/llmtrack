import { z } from 'zod';
import { request } from '@/shared/api/http';
import {
  authorizationInfoSchema,
  mcpSettingsSchema,
  oauthGrantSchema,
} from '@/shared/api/schemas';

/** An assistant's authorization request, as it arrives in the query string. */
export interface AuthorizationRequest {
  response_type: string;
  client_id: string;
  redirect_uri: string;
  code_challenge: string;
  code_challenge_method: string;
  state?: string;
  resource?: string;
}

/** Whether the server answers MCP clients, and at which URL. */
export const getMcpSettings = () =>
  request('GET', '/api/settings/mcp', mcpSettingsSchema);

/** Verifies an assistant's authorization request and says who is asking. */
export const getAuthorization = (req: AuthorizationRequest) => {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(req)) {
    if (value !== undefined) query.set(key, value);
  }
  return request(
    'GET',
    `/api/oauth/authorize?${query}`,
    authorizationInfoSchema,
  );
};

/** The assistants connected to the signed-in user's account. */
export const listGrants = () =>
  request('GET', '/api/oauth/grants', z.array(oauthGrantSchema));
