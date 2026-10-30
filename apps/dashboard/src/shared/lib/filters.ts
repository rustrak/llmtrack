/**
 * Include/exclude list filters, as the server takes them (`apps/server`
 * "Filters"): `team_id=3,8` keeps only those, `exclude_key_id=17` leaves one
 * out. A page's URL holds them the same way, under its own names.
 */

/** A list in the URL: `3`, `gpt-4o` or `3,8,12`. */
export type ListParam = string | number;

/** The params of filters `N`: each with its `exclude_` twin. */
export type FilterParams<N extends string> = {
  [K in N | `exclude_${N}`]?: ListParam;
};

/** One filter in the UI: these values, kept or left out. */
export interface ListFilter {
  values: string[];
  exclude: boolean;
}

const ID = /^[1-9]\d*$/;

/** `3`, `'3,8'` or `[3, 8]` as distinct trimmed values; ids keeps only ids. */
export function parseList(value: unknown, ids = false): string[] {
  const items = Array.isArray(value)
    ? value
    : typeof value === 'string'
      ? value.split(',')
      : typeof value === 'number'
        ? [value]
        : [];
  const values = items
    .filter((v) => typeof v === 'string' || typeof v === 'number')
    .map((v) => String(v).trim())
    .filter((v) => v !== '' && (!ids || ID.test(v)));
  return [...new Set(values)];
}

/**
 * The values as one param. A lone id stays a number so the router writes
 * `team=3`, not `team="3"`; none is `undefined`, which drops the param.
 */
export function listParam(values: readonly string[]): ListParam | undefined {
  if (values.length === 0) return undefined;
  if (values.length === 1 && ID.test(values[0])) return Number(values[0]);
  return values.join(',');
}

/** The filter `name` of a search. Kept values win over left-out ones. */
export function readFilter(
  search: object,
  name: string,
  ids = false,
): ListFilter {
  const params = search as Record<string, unknown>;
  const kept = parseList(params[name], ids);
  if (kept.length > 0) return { values: kept, exclude: false };
  return { values: parseList(params[`exclude_${name}`], ids), exclude: true };
}

/** The params for `filter` under `name`, clearing its twin. */
export function filterParams<N extends string>(
  name: N,
  filter: ListFilter,
): FilterParams<N> {
  const param = listParam(filter.values);
  return {
    [name]: filter.exclude ? undefined : param,
    [`exclude_${name}`]: filter.exclude ? param : undefined,
  } as FilterParams<N>;
}

/** The value when exactly one is kept: a filter that pins one thing. */
export function onlyValue(param: ListParam | undefined): string | undefined {
  const values = parseList(param);
  return values.length === 1 ? values[0] : undefined;
}

/**
 * A page's filters renamed for the API (`{ team: 'team_id' }` turns
 * `team=3&exclude_key=4` into `team_id=3&exclude_key_id=4`).
 */
export function filterQuery<M extends Record<string, string>>(
  search: object,
  names: M,
): FilterParams<M[keyof M]> {
  return Object.assign(
    {},
    ...Object.entries(names).map(([from, to]) =>
      filterParams(to, readFilter(search, from)),
    ),
  );
}
