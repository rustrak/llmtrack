import { Button, Input, RefreshIcon, Tag } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { syncCatalog } from '@/features/model/api/mutations';
import { updateSettings } from '@/features/settings/api/mutations';
import type { CatalogStatus } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { SettingRow, SettingSection } from '@/shared/ui/components/setting-row';
import { useMoney } from '@/shared/ui/hooks/use-money';

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

  return (
    <>
      <SettingSection title={t('pricing.listTitle')}>
        <SettingRow title={t('pricing.source')}>
          <Tag
            variant="soft"
            tone={status.source === 'synced' ? 'brand' : 'neutral'}
          >
            {status.source === 'synced'
              ? t('pricing.syncedSource')
              : t('pricing.builtin')}
          </Tag>
        </SettingRow>
        <SettingRow title={t('pricing.entries')}>
          <p className="font-mono text-mono text-fg tabular-nums">
            {money.compact(status.entries)}
          </p>
        </SettingRow>
        <SettingRow title={t('pricing.lastSync')}>
          <p className="text-value text-fg">
            {status.synced_at
              ? money.date(status.synced_at)
              : t('pricing.never')}
          </p>
        </SettingRow>
        <SettingRow
          title={t('pricing.url')}
          description={t('pricing.urlHint')}
          htmlFor="catalog-url"
          wide
        >
          <div className="flex items-center gap-2">
            <Input
              id="catalog-url"
              boxClassName="min-w-0 flex-1"
              className="font-mono"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
            />
            <Button
              variant="primary"
              icon={RefreshIcon}
              loading={syncing}
              onClick={sync}
            >
              {t('pricing.sync')}
            </Button>
          </div>
        </SettingRow>
      </SettingSection>
      <SettingSection title={t('pricing.howTitle')}>
        <div className="space-y-2 px-5 py-4 text-body text-fg-subtle">
          <p>{t('pricing.howCatalog')}</p>
          <p>{t('pricing.howCustom')}</p>
          <p>{t('pricing.howCache')}</p>
        </div>
      </SettingSection>
    </>
  );
}
