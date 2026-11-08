import { Button, DeleteIcon, Input, Tooltip } from '@rustrak/ui';
import type { UseFormReturn } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import type { CatalogEntry } from '@/shared/api/schemas';
import {
  FormControl,
  FormField,
  FormItem,
  FormMessage,
} from '@/shared/ui/components/form';
import type { ModelFormValues } from '../../model/model-form';
import { CatalogEntryLine } from './pricing-section';

/**
 * One row per model being added: the public name clients send, the id the
 * provider gets, and the price it will be billed at.
 */
export function MappingsTable({
  form,
  entries,
  removable,
  onEntry,
}: {
  form: UseFormReturn<ModelFormValues>;
  entries: ReadonlyMap<string, CatalogEntry>;
  removable: boolean;
  /** A price-list entry picked by hand, so its rates can be shown. */
  onEntry: (entry: CatalogEntry) => void;
}) {
  const t = useTranslations('models');
  const mappings = form.watch('mappings');
  const mode = form.watch('pricing_mode');
  if (mappings.length === 0) return null;

  return (
    <div className="overflow-hidden rounded-xl border border-border-subtle bg-surface">
      <div className="grid grid-cols-[1fr_1fr_auto] gap-2 border-border-subtle border-b bg-surface-raised px-3 py-2 text-column text-fg-meta">
        <span>{t('mappings.publicName')}</span>
        <span>{t('mappings.providerModel')}</span>
        <span className="w-9" />
      </div>
      {mappings.map((row, index) => {
        const entry = row.catalog_key
          ? entries.get(row.catalog_key)
          : undefined;
        return (
          <div
            key={`${row.catalog_key ?? 'custom'}-${index}`}
            className="space-y-1.5 border-border-divider border-b px-3 py-2.5 last:border-b-0"
          >
            <div className="grid grid-cols-[1fr_1fr_auto] items-start gap-2">
              <FormField
                control={form.control}
                name={`mappings.${index}.public_name`}
                render={({ field }) => (
                  <FormItem>
                    <FormControl>
                      <Input
                        className="font-mono"
                        aria-label={t('mappings.publicName')}
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name={`mappings.${index}.upstream_model`}
                render={({ field }) => (
                  <FormItem>
                    <FormControl>
                      <Input
                        className="font-mono"
                        aria-label={t('mappings.providerModel')}
                        {...field}
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              {removable ? (
                <Tooltip
                  content={t('mappings.remove', { name: row.public_name })}
                >
                  <Button
                    type="button"
                    variant="ghost"
                    size="md"
                    icon={DeleteIcon}
                    aria-label={t('mappings.remove', {
                      name: row.public_name,
                    })}
                    onClick={() =>
                      form.setValue(
                        'mappings',
                        mappings.filter((_, i) => i !== index),
                      )
                    }
                  />
                </Tooltip>
              ) : (
                <span className="w-9" />
              )}
            </div>
            {mode === 'catalog' && (
              <CatalogEntryLine
                catalogKey={row.catalog_key}
                entry={entry}
                onPick={(picked) => {
                  if (picked) onEntry(picked);
                  form.setValue(
                    `mappings.${index}.catalog_key`,
                    picked?.key ?? null,
                  );
                }}
              />
            )}
            {mode === 'free' && (
              <p className="text-hint text-fg-subtle">
                {t('pricingMode.freeRow')}
              </p>
            )}
          </div>
        );
      })}
    </div>
  );
}
