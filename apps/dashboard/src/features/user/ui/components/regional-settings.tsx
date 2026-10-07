import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { User } from '@/shared/api/schemas';
import { intl } from '@/shared/i18n/intl';
import { LOCALES } from '@/shared/i18n/routing';
import { CURRENCIES } from '@/shared/lib/format';
import { Choice } from '@/shared/ui/components/choice';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import { updateProfile } from '../../api/mutations';
import { listTimeZones } from '../../model/time-zones';

const BROWSER = 'browser';

/**
 * Language, time zone and currency. Saving reloads the catalogue and the formats at
 * once, so the change shows without a page reload (the step Rustrak skips
 * for languages).
 */
export function RegionalSettings({ user }: { user: User }) {
  const t = useTranslations('locale');
  const router = useRouter();
  const [pending, setPending] = useState(false);
  const [rate, setRate] = useState(String(user.currency_rate ?? ''));
  const currency = user.currency ?? 'USD';

  const save = async (changes: {
    language?: string | null;
    timezone?: string | null;
    currency?: string;
    currency_rate?: number;
  }) => {
    setPending(true);
    const result = await updateProfile(changes);
    setPending(false);
    if (!result.success) {
      toast.error(t('saveFailed'));
      return;
    }
    await intl.reload();
    await router.invalidate();
  };

  return (
    <div className="space-y-6">
      <div className="space-y-1.5">
        <Label>{t('label')}</Label>
        <Choice
          aria-label={t('label')}
          disabled={pending}
          value={user.language ?? BROWSER}
          onChange={(value) =>
            save({ language: value === BROWSER ? null : value })
          }
          options={[
            { value: BROWSER, label: t('followBrowser') },
            ...LOCALES.map((locale) => ({ value: locale, label: t(locale) })),
          ]}
        />
        <p className="text-xs text-muted-foreground">{t('storedOnAccount')}</p>
      </div>
      <div className="space-y-1.5">
        <Label>{t('timeZoneLabel')}</Label>
        <Choice
          aria-label={t('timeZoneLabel')}
          disabled={pending}
          value={user.timezone ?? BROWSER}
          onChange={(value) =>
            save({ timezone: value === BROWSER ? null : value })
          }
          options={[
            { value: BROWSER, label: t('followBrowser') },
            ...listTimeZones().map((zone) => ({ value: zone, label: zone })),
          ]}
        />
        <p className="text-xs text-muted-foreground">{t('timeZoneHint')}</p>
      </div>
      <div className="space-y-1.5">
        <Label>{t('currencyLabel')}</Label>
        <Choice
          aria-label={t('currencyLabel')}
          disabled={pending}
          value={currency}
          onChange={(currency) => save({ currency })}
          options={CURRENCIES.map((code) => ({ value: code, label: t(code) }))}
        />
        <p className="text-xs text-muted-foreground">{t('currencyHint')}</p>
      </div>
      {currency !== 'USD' && (
        <div className="space-y-1.5">
          <Label htmlFor="currency-rate">{t('rateLabel', { currency })}</Label>
          <Input
            id="currency-rate"
            className="max-w-40"
            inputMode="decimal"
            placeholder="0.86"
            disabled={pending}
            value={rate}
            onChange={(event) => setRate(event.target.value)}
            onBlur={() => {
              const value = Number(rate.replace(',', '.'));
              if (value > 0 && value !== user.currency_rate) {
                save({ currency_rate: value });
              }
            }}
          />
          <p className="text-xs text-muted-foreground">{t('rateHint')}</p>
        </div>
      )}
    </div>
  );
}
