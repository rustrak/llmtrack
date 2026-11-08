/**
 * The URL side of the server's paged lists (`models/list.rs`):
 * `?page=2&per_page=50&sort=-created_at&q=prod`, plus each list's own
 * filters. The same names go to the API, so a screen's search is its query.
 */
export interface ListParams {
  page?: number;
  per_page?: number;
  /** A field, `-` first for descending; unset is the list's default. */
  sort?: string;
  q?: string;
}

export const DEFAULT_PAGE_SIZE = 50;
/** The most the server returns at once: what option lists ask for. */
export const MAX_PAGE_SIZE = 200;

const positive = (value: unknown) => {
  const n = Number(value);
  return Number.isInteger(n) && n > 0 ? n : undefined;
};
const text = (value: unknown) =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined;

/** For `validateSearch`: keeps what is valid, drops the rest. */
export function parseListParams(search: Record<string, unknown>): ListParams {
  const page = positive(search.page);
  const perPage = positive(search.per_page);
  return {
    page: page && page > 1 ? page : undefined,
    per_page:
      perPage && perPage !== DEFAULT_PAGE_SIZE && perPage <= MAX_PAGE_SIZE
        ? perPage
        : undefined,
    sort: text(search.sort),
    q: text(search.q),
  };
}

/** `?a=1&b=x` from the set values, `''` when none is. */
export function toQuery(
  params: Record<string, string | number | undefined>,
): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== '') query.set(key, String(value));
  }
  const s = query.toString();
  return s ? `?${s}` : '';
}
