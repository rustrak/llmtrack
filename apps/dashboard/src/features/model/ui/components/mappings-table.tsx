import { Trash2 } from 'lucide-react';
import type { UseFormReturn } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import type { CatalogEntry } from '@/shared/api/schemas';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  FormControl,
  FormField,
  FormItem,
  FormMessage,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
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
    <div className="rounded-lg border">
      <div className="grid grid-cols-[1fr_1fr_auto] gap-2 border-b px-3 py-2 text-xs font-medium text-muted-foreground">
        <span>{t('mappings.publicName')}</span>
        <span>{t('mappings.providerModel')}</span>
        <span className="w-8" />
      </div>
      {mappings.map((row, index) => {
        const entry = row.catalog_key
          ? entries.get(row.catalog_key)
          : undefined;
        return (
          <div
            key={`${row.catalog_key ?? 'custom'}-${index}`}
            className="space-y-1 border-b px-3 py-2 last:border-b-0"
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
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t('mappings.remove', { name: row.public_name })}
                  onClick={() =>
                    form.setValue(
                      'mappings',
                      mappings.filter((_, i) => i !== index),
                    )
                  }
                >
                  <Trash2 />
                </Button>
              ) : (
                <span className="w-8" />
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
              <p className="text-xs text-muted-foreground">
                {t('pricingMode.freeRow')}
              </p>
            )}
          </div>
        );
      })}
    </div>
  );
}
