import { Input, SearchIcon } from '@rustrak/ui';
import { useState } from 'react';
import type { UseFormReturn } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import type { CatalogEntry } from '@/shared/api/schemas';
import { searchCatalog } from '../../api/queries';
import {
  type ModelFormValues,
  PRICING_MODES,
  type PricingMode,
  ratesFrom,
} from '../../model/model-form';
import { PriceSummary } from './price-summary';
import { RateFields } from './rate-fields';

/**
 * How the models are billed, in plain sight: by the price list (each
 * row shows the entry it matched, and it can be changed), by own rates, or
 * not at all.
 */
export function PricingSection({
  form,
  entries,
}: {
  form: UseFormReturn<ModelFormValues>;
  entries: ReadonlyMap<string, CatalogEntry>;
}) {
  const t = useTranslations('models');
  const mode = form.watch('pricing_mode');
  const choose = (next: PricingMode) => {
    form.setValue('pricing_mode', next);
    // Own rates start from the list's: usually a small edit.
    const first = form.getValues('mappings')[0]?.catalog_key;
    const listed = first ? entries.get(first) : undefined;
    if (next === 'custom' && listed && !form.getValues('rates.input')) {
      form.setValue('rates', ratesFrom(listed.pricing));
    }
  };
  return (
    <div className="space-y-3">
      <fieldset className="grid gap-2 sm:grid-cols-3">
        <legend className="sr-only">{t('pricingMode.title')}</legend>
        {PRICING_MODES.map((value) => (
          <label
            key={value}
            className="cursor-pointer rounded-lg border border-border-subtle bg-surface px-3 py-2.5 transition-[background-color,border-color] duration-fast hover:bg-surface-hover has-[:checked]:border-border-brand has-[:checked]:bg-surface-brand/10 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-ring"
          >
            <input
              type="radio"
              name="pricing_mode"
              value={value}
              checked={mode === value}
              onChange={() => choose(value)}
              className="sr-only"
            />
            <span className="block text-control text-fg">
              {t(`pricingMode.${value}`)}
            </span>
            <span className="mt-0.5 block text-hint text-fg-subtle">
              {t(`pricingMode.${value}Hint`)}
            </span>
          </label>
        ))}
      </fieldset>
      {mode === 'custom' && <RateFields control={form.control} />}
    </div>
  );
}

/**
 * The price-list entry one row is billed by, with a way to pick another or
 * go back to the automatic match.
 */
export function CatalogEntryLine({
  catalogKey,
  entry,
  onPick,
}: {
  catalogKey: string | null;
  entry: CatalogEntry | undefined;
  onPick: (entry: CatalogEntry | null) => void;
}) {
  const t = useTranslations('models');
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [hits, setHits] = useState<CatalogEntry[]>([]);
  const search = async (q: string) => {
    setQuery(q);
    const result = q.trim() ? await searchCatalog(q) : null;
    setHits(result?.success ? result.data : []);
  };
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-hint">
        {catalogKey ? (
          <>
            <span className="text-fg-subtle">{t('pricingMode.billedBy')}</span>
            <code className="font-mono text-mono-sm text-fg">{catalogKey}</code>
            {entry && <PriceSummary pricing={entry.pricing} />}
          </>
        ) : (
          <span className="text-sev-warning-fg">{t('mappings.unpriced')}</span>
        )}
        <button
          type="button"
          className="text-fg-brand hover:underline"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          {t('pricingMode.change')}
        </button>
      </div>
      {open && (
        <div className="space-y-1 rounded-lg border border-border-subtle bg-surface-raised p-1.5">
          <Input
            size="sm"
            leading={<SearchIcon size="md" />}
            className="font-mono"
            autoFocus
            value={query}
            placeholder={t('pricingMode.search')}
            aria-label={t('pricingMode.search')}
            onChange={(e) => search(e.target.value)}
          />
          <button
            type="button"
            className="flex h-menu-item w-full items-center rounded-md px-2.5 text-left text-control text-fg-muted hover:bg-surface-selected hover:text-fg"
            onClick={() => {
              onPick(null);
              setOpen(false);
            }}
          >
            {t('pricingMode.automatic')}
          </button>
          {hits.map((hit) => (
            <button
              key={hit.key}
              type="button"
              className="flex min-h-menu-item w-full flex-wrap items-center justify-between gap-2 rounded-md px-2.5 py-1 text-left hover:bg-surface-selected"
              onClick={() => {
                onPick(hit);
                setOpen(false);
              }}
            >
              <code className="font-mono text-mono-sm text-fg">{hit.key}</code>
              <PriceSummary pricing={hit.pricing} />
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
