/**
 * Every path the Rust server owns. Every other path belongs to the router.
 * `apps/server/src/routes/dashboard.rs` keeps the same list in
 * `API_PREFIXES`, and a test holds the two together.
 */
export const API_PREFIXES = ['/api', '/auth', '/health', '/v1'] as const;
