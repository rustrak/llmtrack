import { Button, Input, NewIcon, SearchIcon, SelectedIcon } from '@rustrak/ui';
import { useEffect, useMemo, useState } from 'react';
import { useTranslations } from 'use-intl';
import type { CatalogEntry } from '@/shared/api/schemas';
import { cn } from '@/shared/lib/utils';
import { listProviderModels } from '../../api/queries';
import { PriceSummary } from './price-summary';

/**
 * The provider's models from the price
 * list to tick, plus any model id typed by hand (a fine-tune, a vLLM
 * deployment). `single` replaces the pick instead of adding to it.
 */
export function ProviderModelPicker({
  provider,
  picked,
  onToggle,
  onCustom,
  onLoaded,
}: {
  provider: string;
  picked: readonly (string | null)[];
  onToggle: (entry: CatalogEntry) => void;
  onCustom: (model: string) => void;
  onLoaded: (entries: CatalogEntry[]) => void;
}) {
  const t = useTranslations('models');
  const [entries, setEntries] = useState<CatalogEntry[]>([]);
  const [query, setQuery] = useState('');
  const [custom, setCustom] = useState('');

  useEffect(() => {
    let current = true;
    setEntries([]);
    listProviderModels(provider).then((result) => {
      if (!current || !result.success) return;
      setEntries(result.data);
      onLoaded(result.data);
    });
    return () => {
      current = false;
    };
  }, [provider, onLoaded]);

  const shown = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return entries.filter((e) =>
      words.every((w) => e.model.toLowerCase().includes(w)),
    );
  }, [entries, query]);

  const addCustom = () => {
    if (custom.trim()) onCustom(custom.trim());
    setCustom('');
  };

  return (
    <div className="space-y-2">
      {entries.length > 0 && (
        <>
          <Input
            leading={<SearchIcon size="md" />}
            className="font-mono"
            placeholder={t('picker.search', { count: entries.length })}
            aria-label={t('picker.searchLabel')}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <div className="max-h-56 divide-y divide-border-divider overflow-y-auto rounded-lg border border-border-subtle bg-surface">
            {shown.map((entry) => {
              const selected = picked.includes(entry.key);
              return (
                <button
                  key={entry.key}
                  type="button"
                  onClick={() => onToggle(entry)}
                  className={cn(
                    'flex w-full items-start gap-2.5 px-3 py-2 text-left transition-[background-color] duration-fast hover:bg-surface-hover',
                    selected && 'bg-surface-selected',
                  )}
                >
                  <SelectedIcon
                    size="md"
                    className={cn(
                      'mt-0.5 shrink-0 text-fg-brand',
                      !selected && 'invisible',
                    )}
                  />
                  <div className="min-w-0 space-y-0.5">
                    <p className="truncate font-mono text-mono text-fg">
                      {entry.model}
                    </p>
                    <PriceSummary pricing={entry.pricing} />
                  </div>
                </button>
              );
            })}
            {shown.length === 0 && (
              <p className="px-3 py-4 text-center text-meta text-fg-subtle">
                {t('picker.noMatch')}
              </p>
            )}
          </div>
        </>
      )}
      <div className="flex gap-2">
        <Input
          boxClassName="flex-1"
          className="font-mono"
          placeholder={
            entries.length > 0
              ? t('picker.customPlaceholder')
              : t('picker.customOnly')
          }
          aria-label={t('picker.customLabel')}
          value={custom}
          onChange={(event) => setCustom(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              addCustom();
            }
          }}
        />
        <Button
          type="button"
          variant="secondary"
          icon={NewIcon}
          onClick={addCustom}
          disabled={!custom.trim()}
        >
          {t('picker.addCustom')}
        </Button>
      </div>
    </div>
  );
}
