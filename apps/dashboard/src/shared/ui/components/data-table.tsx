import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Check,
  ChevronLeft,
  ChevronRight,
  PlusCircle,
  Search,
  X,
} from 'lucide-react';
import { type ReactNode, useEffect, useState } from 'react';
import { useFormatter, useTranslations } from 'use-intl';
import {
  DEFAULT_PAGE_SIZE,
  nextSort,
  PAGE_SIZES,
  pageRange,
  sortOf,
} from '@/shared/lib/list-params';
import { cn } from '@/shared/lib/utils';
import { Choice } from './choice';
import { Button } from './shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from './shadcn/dropdown-menu';
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from './shadcn/input-group';

/**
 * A server-paged table that fills what the page leaves: the toolbar and the
 * pagination stay put, the rows scroll under a sticky header.
 */
export function DataTable({
  toolbar,
  head,
  children,
  empty,
  footer,
}: {
  toolbar?: ReactNode;
  /** The header cells, one `<th>` each (`ColumnHead`, `SortHead`). */
  head: ReactNode;
  /** The rows. */
  children?: ReactNode;
  /** Shown instead of rows, centred in the height they would have had. */
  empty?: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      {toolbar && (
        <div className="flex flex-wrap items-center gap-2">{toolbar}</div>
      )}
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-card shadow-xs">
        <div className="flex min-h-0 flex-1 flex-col overflow-auto">
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10 bg-card shadow-[inset_0_-1px_0_var(--color-border)]">
              <tr>{head}</tr>
            </thead>
            <tbody className="[&_tr:last-child]:border-0">
              {!empty && children}
            </tbody>
          </table>
          {empty && (
            <div className="flex flex-1 items-center justify-center px-6 py-16">
              {empty}
            </div>
          )}
        </div>
        {footer && (
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-t px-4 py-2.5">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}

const headClass =
  'h-10 px-4 text-left align-middle text-xs font-medium whitespace-nowrap text-muted-foreground';

export function ColumnHead({
  children,
  className,
}: {
  children?: ReactNode;
  className?: string;
}) {
  return <th className={cn(headClass, className)}>{children}</th>;
}

/** A header that sorts by `field`: ascending, descending, then the default. */
export function SortHead({
  field,
  sort,
  fallback,
  onSort,
  children,
  className,
}: {
  field: string;
  sort: string | undefined;
  /** The list's default sort, as the server applies it. */
  fallback: string;
  onSort: (sort: string | undefined) => void;
  children: ReactNode;
  className?: string;
}) {
  const dir = sortOf(sort, field, fallback);
  const Icon =
    dir === 'asc' ? ArrowUp : dir === 'desc' ? ArrowDown : ArrowUpDown;
  return (
    <th
      className={cn(headClass, 'px-2', className)}
      aria-sort={
        dir === 'asc' ? 'ascending' : dir === 'desc' ? 'descending' : 'none'
      }
    >
      <button
        type="button"
        onClick={() => onSort(nextSort(sort, field, fallback))}
        className={cn(
          'group/sort -mx-0 inline-flex h-7 items-center gap-1 rounded-md px-2 transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none',
          dir && 'text-foreground',
        )}
      >
        {children}
        <Icon
          className={cn(
            'size-3.5 transition-opacity',
            dir ? 'opacity-100' : 'opacity-0 group-hover/sort:opacity-60',
          )}
        />
      </button>
    </th>
  );
}

/** A body row; `onOpen` makes the whole row a way into its detail. */
export function DataRow({
  children,
  onOpen,
  className,
}: {
  children: ReactNode;
  onOpen?: () => void;
  className?: string;
}) {
  return (
    <tr
      onClick={(event) => {
        const target = event.target as HTMLElement;
        // Menus and panels opened from the row are portals elsewhere in the
        // page, yet React still bubbles their clicks here; and clicks on the
        // row's own links and buttons are theirs.
        if (!event.currentTarget.contains(target)) return;
        if (target.closest('a,button,input,label')) return;
        onOpen?.();
      }}
      className={cn(
        'group/row border-b transition-colors hover:bg-muted/40',
        onOpen && 'cursor-pointer',
        className,
      )}
    >
      {children}
    </tr>
  );
}

export function Cell({
  children,
  className,
}: {
  children?: ReactNode;
  className?: string;
}) {
  return (
    <td className={cn('h-14 px-4 align-middle', className)}>{children}</td>
  );
}

/** Row actions, quiet until the row is hovered or focused. */
export function ActionsCell({ children }: { children: ReactNode }) {
  return (
    <td className="w-12 px-2 text-right align-middle">
      <div className="opacity-0 transition-opacity group-hover/row:opacity-100 focus-within:opacity-100 has-aria-expanded:opacity-100 pointer-coarse:opacity-100">
        {children}
      </div>
    </td>
  );
}

/** What fills the table when no row does: `DataTable`'s `empty`. */
export function DataTableEmpty({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="text-center">
      <p className="font-medium">{title}</p>
      {description && (
        <p className="mx-auto mt-1 max-w-sm text-sm text-muted-foreground">
          {description}
        </p>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

/** Free-text search that waits for a pause in typing before it applies. */
export function SearchField({
  value,
  onChange,
  placeholder,
}: {
  value: string | undefined;
  onChange: (value: string | undefined) => void;
  placeholder: string;
}) {
  const [draft, setDraft] = useState(value ?? '');
  useEffect(() => setDraft(value ?? ''), [value]);
  useEffect(() => {
    const next = draft.trim() || undefined;
    if (next === value) return;
    const timer = setTimeout(() => onChange(next), 300);
    return () => clearTimeout(timer);
  }, [draft, value, onChange]);

  return (
    <InputGroup className="w-full sm:w-72">
      <InputGroupAddon>
        <Search />
      </InputGroupAddon>
      <InputGroupInput
        type="search"
        value={draft}
        placeholder={placeholder}
        aria-label={placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') setDraft('');
        }}
      />
    </InputGroup>
  );
}

/**
 * A one-of filter as a pill: dashed `+ Team` while unset, `Team: Acme` once
 * chosen, with its own clear button.
 */
export function FilterPill({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string | undefined;
  options: readonly { value: string; label: string }[];
  onChange: (value: string | undefined) => void;
}) {
  const t = useTranslations('table');
  const chosen = options.find((o) => o.value === value);
  return (
    <div
      className={cn(
        'flex h-8 items-center rounded-md border text-sm transition-colors',
        chosen ? 'border-solid bg-muted/50' : 'border-dashed',
      )}
    >
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <button
              type="button"
              className="flex h-full items-center gap-1.5 rounded-md px-2.5 text-muted-foreground transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
            />
          }
        >
          {!chosen && <PlusCircle className="size-3.5" />}
          {label}
          {chosen && (
            <>
              <span className="h-4 w-px bg-border" />
              <span className="font-medium text-foreground">
                {chosen.label}
              </span>
            </>
          )}
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-52">
          <DropdownMenuGroup>
            <DropdownMenuLabel>{label}</DropdownMenuLabel>
            {options.map((option) => (
              <DropdownMenuItem
                key={option.value}
                onClick={() =>
                  onChange(option.value === value ? undefined : option.value)
                }
              >
                <Check
                  className={cn(
                    'size-4',
                    option.value === value ? 'opacity-100' : 'opacity-0',
                  )}
                />
                {option.label}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
          {chosen && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuItem onClick={() => onChange(undefined)}>
                {t('clearFilter')}
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
      {chosen && (
        <button
          type="button"
          aria-label={t('clearFilter')}
          onClick={() => onChange(undefined)}
          className="mr-1 flex size-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <X className="size-3.5" />
        </button>
      )}
    </div>
  );
}

/** `1–50 of 312`, rows per page, and the page buttons. */
export function Pagination({
  page,
  perPage,
  total,
  onPage,
  onPerPage,
}: {
  page: number;
  perPage: number;
  total: number;
  onPage: (page: number) => void;
  onPerPage: (perPage: number) => void;
}) {
  const t = useTranslations('table');
  const format = useFormatter();
  const pageCount = Math.max(1, Math.ceil(total / perPage));
  const from = total === 0 ? 0 : (page - 1) * perPage + 1;
  const to = Math.min(page * perPage, total);
  const n = (value: number) => format.number(value);

  return (
    <>
      <p className="text-sm text-muted-foreground tabular-nums">
        {t('range', { from: n(from), to: n(to), total: n(total) })}
      </p>
      <div className="flex items-center gap-4">
        <div className="hidden items-center gap-2 text-sm text-muted-foreground sm:flex">
          {t('perPage')}
          <Choice
            aria-label={t('perPage')}
            className="h-8 w-18"
            value={String(perPage)}
            onChange={(value) => onPerPage(Number(value))}
            options={PAGE_SIZES.map((size) => ({
              value: String(size),
              label: String(size),
            }))}
          />
        </div>
        <nav aria-label={t('pages')} className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={t('previous')}
            disabled={page <= 1}
            onClick={() => onPage(page - 1)}
          >
            <ChevronLeft />
          </Button>
          {pageRange(page, pageCount).map((p, i) =>
            p === null ? (
              <span
                key={`gap-${i}`}
                className="w-6 text-center text-muted-foreground"
              >
                …
              </span>
            ) : (
              <Button
                key={p}
                variant={p === page ? 'outline' : 'ghost'}
                size="icon-sm"
                aria-current={p === page ? 'page' : undefined}
                className="tabular-nums"
                onClick={() => onPage(p)}
              >
                {n(p)}
              </Button>
            ),
          )}
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={t('next')}
            disabled={page >= pageCount}
            onClick={() => onPage(page + 1)}
          >
            <ChevronRight />
          </Button>
        </nav>
      </div>
    </>
  );
}

/**
 * `Pagination` for a page of a server list, or nothing when it is empty.
 * `onChange` gets URL search changes: the first page and the default size
 * are left out of the URL.
 */
export function ListFooter({
  list,
  onChange,
}: {
  list: { page: number; per_page: number; total: number };
  onChange: (patch: { page?: number; per_page?: number }) => void;
}) {
  if (list.total === 0) return null;
  return (
    <Pagination
      page={list.page}
      perPage={list.per_page}
      total={list.total}
      onPage={(page) => onChange({ page: page > 1 ? page : undefined })}
      onPerPage={(perPage) =>
        onChange({
          page: undefined,
          per_page: perPage === DEFAULT_PAGE_SIZE ? undefined : perPage,
        })
      }
    />
  );
}
