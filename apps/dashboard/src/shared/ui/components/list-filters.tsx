import {
  Button,
  Checkbox,
  CloseIcon,
  Menu,
  NewIcon,
  Popover,
  QueryBar,
  type QueryField,
  SegmentedControl,
  SegmentedItem,
  Tooltip,
} from '@rustrak/ui';
import { type ReactNode, useMemo, useState } from 'react';
import { useTranslations } from 'use-intl';
import type { ListFilter, ListParam } from '@/shared/lib/filters';
import { barFilters, barPatch } from '@/shared/lib/table-query';
import { cn } from '@/shared/lib/utils';

/*
 * A list's toolbar: Rustrak's query bar for search and `key:value` filters,
 * and beside it the filter pills llmtrack always had, one per filter, for
 * whoever prefers to click. Both write the same URL params.
 */

/** A filter of a list: the URL param it writes, and the values it offers. */
export interface ListField extends Omit<QueryField, 'key'> {
  /** The URL param: `team_id`, `status`. */
  key: string;
  /**
   * Also offer leaving these values out, as `exclude_<key>` (the server's own
   * name for it): "Team ≠ Acme".
   */
  exclude?: boolean;
}

/**
 * A list's search and filters, as Rustrak's query bar: `team_id:3 status:active
 * prod` in one field, with suggestions. It reads the page's search and hands
 * back a patch of the same params, so the loader and the API are untouched.
 */
export function ListQueryBar({
  fields,
  search,
  onChange,
  placeholder,
  className,
}: {
  fields: ListField[];
  /** The page's validated search: `q` and the fields' params. */
  search: object & { q?: string };
  onChange: (patch: Record<string, ListParam | undefined>) => void;
  placeholder?: string;
  className?: string;
}) {
  const t = useTranslations('table');
  const barFields = useMemo<QueryField[]>(
    () =>
      fields.flatMap(({ exclude, ...field }) => [
        field,
        ...(exclude
          ? [
              {
                ...field,
                key: `exclude_${field.key}`,
                label: t('excludedField', { field: field.label }),
              },
            ]
          : []),
      ]),
    [fields, t],
  );
  const keys = barFields.map((field) => field.key);

  return (
    <QueryBar
      className={className}
      fields={barFields}
      filters={barFilters(keys, search)}
      search={search.q ?? ''}
      placeholder={placeholder}
      onChange={(next) => onChange(barPatch(keys, next))}
    />
  );
}

/** The pill a filter opens from: dashed while unset, filled once chosen. */
function PillTrigger({
  chosen,
  children,
  ...props
}: { chosen: boolean; children: ReactNode } & React.ComponentProps<'button'>) {
  return (
    <button
      type="button"
      {...props}
      className={cn(
        'flex h-full min-w-0 items-center gap-1.5 rounded-md px-2.5 text-control transition-[color] duration-fast focus-visible:outline-2 focus-visible:outline-ring',
        chosen ? 'text-fg-subtle' : 'text-fg-subtle hover:text-fg',
      )}
    >
      {!chosen && <NewIcon size="sm" className="text-fg-ghost" />}
      {children}
    </button>
  );
}

function PillFrame({
  chosen,
  onClear,
  children,
}: {
  chosen: boolean;
  onClear: () => void;
  children: ReactNode;
}) {
  const t = useTranslations('table');
  return (
    <div
      className={cn(
        'flex h-control-md items-center rounded-md border transition-[border-color,background-color] duration-fast',
        chosen
          ? 'border-border bg-surface-active'
          : 'border-border border-dashed hover:border-border-strong',
      )}
    >
      {children}
      {chosen && (
        <Tooltip content={t('clearFilter')}>
          <button
            type="button"
            aria-label={t('clearFilter')}
            onClick={onClear}
            className="mr-1 flex size-5 items-center justify-center rounded-sm text-fg-ghost transition-[background-color,color] duration-fast hover:bg-surface-selected hover:text-fg"
          >
            <CloseIcon size="sm" />
          </button>
        </Tooltip>
      )}
    </div>
  );
}

function PillValue({ children }: { children: ReactNode }) {
  return (
    <>
      <span aria-hidden className="h-3.5 w-px bg-border-strong" />
      {children}
    </>
  );
}

/**
 * A one-of filter as a pill: dashed `+ Team` while unset, `Team | Acme` once
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
    <PillFrame chosen={Boolean(chosen)} onClear={() => onChange(undefined)}>
      <Menu
        popupClassName="w-56"
        trigger={
          <PillTrigger chosen={Boolean(chosen)}>
            {label}
            {chosen && (
              <PillValue>
                <span className="max-w-48 truncate font-medium text-fg">
                  {chosen.label}
                </span>
              </PillValue>
            )}
          </PillTrigger>
        }
        actions={[
          ...options.map((option) => ({
            id: option.value,
            label: option.label,
            selected: option.value === value,
            onSelect: () =>
              onChange(option.value === value ? undefined : option.value),
          })),
          ...(chosen
            ? [
                {
                  id: '__clear',
                  label: t('clearFilter'),
                  icon: CloseIcon,
                  separated: true,
                  onSelect: () => onChange(undefined),
                },
              ]
            : []),
        ]}
      />
    </PillFrame>
  );
}

/**
 * A list filter as a pill: dashed `+ Team` while unset, `Team | Acme +2` once
 * chosen, `Team | not Acme` when left out. The panel picks several values and
 * whether they are kept ("is") or left out ("is not").
 */
export function ListFilterPill({
  label,
  filter,
  options,
  onChange,
}: {
  label: string;
  filter: ListFilter;
  options: readonly { value: string; label: string }[];
  onChange: (filter: ListFilter) => void;
}) {
  const t = useTranslations('table');
  // The mode outlives an empty selection, which the URL cannot hold.
  const [wantsExclude, setWantsExclude] = useState(filter.exclude);
  const exclude = filter.values.length > 0 ? filter.exclude : wantsExclude;
  const chosen = filter.values.length > 0;
  const nameOf = (value: string) =>
    options.find((o) => o.value === value)?.label ?? value;
  const clear = () => onChange({ values: [], exclude });
  const toggle = (value: string, on: boolean) =>
    onChange({
      exclude,
      values: on
        ? [...filter.values, value]
        : filter.values.filter((v) => v !== value),
    });
  // Chosen values that are not offered (deleted, or typed in the URL) stay
  // listed so that they can be unticked.
  const listed = [
    ...options,
    ...filter.values
      .filter((v) => !options.some((o) => o.value === v))
      .map((value) => ({ value, label: value })),
  ];
  return (
    <PillFrame chosen={chosen} onClear={clear}>
      <Popover
        title={label}
        popupClassName="w-64"
        trigger={
          <PillTrigger chosen={chosen}>
            {label}
            {chosen && (
              <PillValue>
                {exclude && <span>{t('filterNot')}</span>}
                <span className="max-w-48 truncate font-medium text-fg">
                  {nameOf(filter.values[0])}
                </span>
                {filter.values.length > 1 && (
                  <span className="font-medium text-fg">
                    {t('filterMore', { count: filter.values.length - 1 })}
                  </span>
                )}
              </PillValue>
            )}
          </PillTrigger>
        }
      >
        <div className="border-border-subtle border-b p-2">
          <SegmentedControl
            aria-label={t('excludeValues', { filter: label })}
            className="w-full [&>*]:flex-1"
            value={exclude ? 'exclude' : 'include'}
            onValueChange={(mode) => {
              const next = mode === 'exclude';
              setWantsExclude(next);
              if (chosen) onChange({ values: filter.values, exclude: next });
            }}
          >
            <SegmentedItem value="include">{t('filterIs')}</SegmentedItem>
            <SegmentedItem value="exclude">{t('filterIsNot')}</SegmentedItem>
          </SegmentedControl>
        </div>
        <div className="flex max-h-72 flex-col overflow-y-auto p-1.5">
          {listed.map((option) => (
            <label
              key={option.value}
              className="flex h-menu-item shrink-0 cursor-pointer items-center gap-2.5 rounded-md px-2.5 text-control text-fg-muted hover:bg-surface-selected hover:text-fg"
            >
              <Checkbox
                aria-label={option.label}
                checked={filter.values.includes(option.value)}
                onCheckedChange={(on) => toggle(option.value, on)}
              />
              <span className="min-w-0 flex-1 truncate">{option.label}</span>
            </label>
          ))}
        </div>
        {chosen && (
          <div className="border-border-subtle border-t p-1.5">
            <Button
              variant="ghost"
              size="sm"
              icon={CloseIcon}
              className="w-full justify-start"
              onClick={clear}
            >
              {t('clearFilter')}
            </Button>
          </div>
        )}
      </Popover>
    </PillFrame>
  );
}
