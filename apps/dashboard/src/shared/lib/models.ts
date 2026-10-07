/**
 * A model name can have several deployments (rows with the same name).
 * Access is granted by name, so pickers show each name once and tick it by
 * any of its ids.
 */
interface Named {
  id: number;
  name: string;
}

export function byName<T extends Named>(models: readonly T[]): T[] {
  const seen = new Set<string>();
  return models.filter((m) => !seen.has(m.name) && seen.add(m.name));
}

export function isPicked(
  models: readonly Named[],
  ids: number[],
  name: string,
) {
  return models.some((m) => m.name === name && ids.includes(m.id));
}

export function toggle(
  models: readonly Named[],
  ids: number[],
  name: string,
  on: boolean,
): number[] {
  const ofName = models.filter((m) => m.name === name).map((m) => m.id);
  const rest = ids.filter((id) => !ofName.includes(id));
  return on ? [...rest, ofName[0]] : rest;
}
