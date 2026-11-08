import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { RegionalSettings } from '@/features/user/ui/components/regional-settings';
import { translator } from '@/shared/i18n/intl';
import { SettingRow, SettingSection } from '@/shared/ui/components/setting-row';
import { SettingsHeader } from '@/shared/ui/components/settings-header';
import { ThemeSelector } from '@/shared/ui/components/theme-selector';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';

export const Route = createFileRoute('/_authenticated/settings/preferences')({
  head: () => ({
    meta: [{ title: translator('settings')('preferences.meta') }],
  }),
  component: PreferencesPage,
});

/**
 * How llmtrack reads to the reader: language, time zone and currency, saved
 * on the account, and the theme, kept on this device.
 */
function PreferencesPage() {
  const t = useTranslations('settings');
  const user = useCurrentUser();
  return (
    <>
      <SettingsHeader
        title={t('preferences.title')}
        description={t('preferences.subtitle')}
      />
      <SettingSection
        title={t('profile.regional')}
        description={t('profile.regionalDescription')}
      >
        <RegionalSettings user={user} />
      </SettingSection>
      <SettingSection title={t('preferences.display')}>
        <SettingRow
          title={t('appearance.theme')}
          description={t('appearance.themeDescription')}
        >
          <ThemeSelector />
        </SettingRow>
      </SettingSection>
    </>
  );
}
