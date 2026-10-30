/**
 * Whether the consent page may send the browser to `url`, the redirect the
 * server built from the client's registration.
 *
 * The server already refuses executable schemes; this is the second lock, so
 * that no answer it gives can run script in the dashboard's origin.
 */
export function navigableRedirect(url: string): boolean {
  try {
    return !['javascript:', 'data:', 'vbscript:', 'blob:', 'file:'].includes(
      new URL(url).protocol,
    );
  } catch {
    return false;
  }
}
