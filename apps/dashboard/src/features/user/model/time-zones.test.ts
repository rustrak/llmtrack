import { describe, expect, it } from 'vitest';
import { listTimeZones } from './time-zones';

describe('listTimeZones', () => {
  it('puts UTC first and lists it once', () => {
    const zones = listTimeZones();
    expect(zones[0]).toBe('UTC');
    expect(zones.filter((z) => z === 'UTC')).toHaveLength(1);
    expect(zones).toContain('Europe/Madrid');
  });
});
