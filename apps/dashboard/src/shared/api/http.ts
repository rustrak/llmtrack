import type { z } from 'zod';

/**
 * Every call answers with a value, never a throw: a screen renders both
 * branches of a `Result`, the way `@rustrak/client` does it, without a
 * separate client package to keep in step.
 */
export type Result<T> =
  | { success: true; data: T }
  | { success: false; error: ApiError };

export type ApiErrorKind =
  | 'network'
  | 'invalid_response'
  | 'unauthenticated'
  | 'forbidden'
  | 'not_found'
  | 'conflict'
  | 'validation'
  | 'rate_limited'
  | 'server';

export interface FieldError {
  field: string;
  code: 'required' | 'invalid' | 'already_exists' | 'too_long';
}

export interface ApiError {
  kind: ApiErrorKind;
  status: number;
  message: string;
  fields: FieldError[];
}

function kindOf(status: number): ApiErrorKind {
  switch (status) {
    case 401:
      return 'unauthenticated';
    case 403:
      return 'forbidden';
    case 404:
      return 'not_found';
    case 409:
      return 'conflict';
    case 429:
      return 'rate_limited';
    default:
      return status >= 500 ? 'server' : 'validation';
  }
}

export async function errorFrom(response: Response): Promise<ApiError> {
  const body = await response.json().catch(() => null);
  return {
    kind: kindOf(response.status),
    status: response.status,
    message: body?.error?.message ?? response.statusText,
    fields: body?.error?.fields ?? [],
  };
}

/**
 * Calls the server on the page's own origin. Without a schema the body is
 * ignored (204s and deletes).
 */
export async function request<T = undefined>(
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
  path: string,
  schema?: z.ZodType<T>,
  body?: unknown,
): Promise<Result<T>> {
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      credentials: 'same-origin',
      headers: body === undefined ? {} : { 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
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
  if (!schema || response.status === 204) {
    return { success: true, data: undefined as T };
  }

  const parsed = schema.safeParse(await response.json().catch(() => null));
  if (!parsed.success) {
    return {
      success: false,
      error: {
        kind: 'invalid_response',
        status: response.status,
        message: `Unexpected response from ${path}`,
        fields: [],
      },
    };
  }
  return { success: true, data: parsed.data };
}

/** The data of a loader's result, or a thrown error for the route's error screen. */
export function unwrap<T>(result: Result<T>): T {
  if (!result.success) throw new Error(result.error.message);
  return result.data;
}
