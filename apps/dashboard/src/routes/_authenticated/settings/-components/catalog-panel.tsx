import { useRouter } from '@tanstack/react-router';
import { RefreshCw } from 'lucide-react';
import type React from 'react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { syncCatalog } from '@/features/model/api/mutations';
import { updateSettings } from '@/features/settings/api/mutations';
import type { CatalogStatus } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { Section } from './section';

/** Where prices come from, and the button that refreshes them. */
export function CatalogPanel({ status }: { status: CatalogStatus }) {
  const t = useTranslations('settings');
  const router = useRouter();
  const money = useMoney();
  const [url, setUrl] = useState(status.url);
  const [syncing, setSyncing] = useState(false);

  const sync = async () => {
    setSyncing(true);
    if (url.trim() !== status.url) {
      const saved = await updateSettings({
        price_catalog_url: url.trim() || null,
      });
      if (!saved.success) {
        setSyncing(false);
        toast.error(readableMessage(saved.error));
        return;
      }
    }
    const result = await syncCatalog();
    setSyncing(false);
    if (!result.success) {
      toast.error(readableMessage(result.error));
      return;
    }
    toast.success(t('pricing.synced', { count: result.data.entries }));
    await router.invalidate();
  };

  const facts: [string, React.ReactNode][] = [
    [
      t('pricing.source'),
      <Badge
        key="source"
        variant={status.source === 'synced' ? 'default' : 'secondary'}
      >
        {status.source === 'synced'
          ? t('pricing.syncedSource')
          : t('pricing.builtin')}
      </Badge>,
    ],
    [t('pricing.entries'), money.compact(status.entries)],
    [
      t('pricing.lastSync'),
      status.synced_at ? money.date(status.synced_at) : t('pricing.never'),
    ],
  ];

  return (
    <>
      <Section title={t('pricing.listTitle')}>
        <dl className="grid grid-cols-3 gap-4 rounded-lg border px-4 py-3">
          {facts.map(([label, value]) => (
            <div key={label} className="space-y-1">
              <dt className="text-xs text-muted-foreground">{label}</dt>
              <dd className="text-sm font-medium tabular-nums">{value}</dd>
            </div>
          ))}
        </dl>
        <div className="space-y-1.5">
          <Label htmlFor="catalog-url">{t('pricing.url')}</Label>
          <div className="flex gap-2">
            <Input
              id="catalog-url"
              className="font-mono text-xs"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
            />
            <Button onClick={sync} disabled={syncing}>
              <RefreshCw className={syncing ? 'animate-spin' : undefined} />
              {t('pricing.sync')}
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">
            {t('pricing.urlHint')}
          </p>
        </div>
      </Section>
      <Section title={t('pricing.howTitle')}>
        <div className="space-y-2 text-sm text-muted-foreground">
          <p>{t('pricing.howCatalog')}</p>
          <p>{t('pricing.howCustom')}</p>
          <p>{t('pricing.howCache')}</p>
        </div>
      </Section>
    </>
  );
}
