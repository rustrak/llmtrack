/** Every IANA zone this browser knows, UTC first. */
export function listTimeZones(): string[] {
  let zones: string[] = [];
  try {
    zones = Intl.supportedValuesOf('timeZone');
  } catch {
    // Older engines: UTC alone is still a valid answer.
  }
  return ['UTC', ...zones.filter((zone) => zone !== 'UTC')];
}
