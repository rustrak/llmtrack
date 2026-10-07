import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { translator } from '@/shared/i18n/intl';
import { ThemeSelector } from '@/shared/ui/components/theme-selector';
import { Section, SettingsHeader } from './-components/section';

export const Route = createFileRoute('/_authenticated/settings/appearance')({
  head: () => ({
    meta: [{ title: translator('settings')('appearance.meta') }],
  }),
  component: AppearancePage,
});

function AppearancePage() {
  const t = useTranslations('settings');
  return (
    <div>
      <SettingsHeader
        title={t('appearance.title')}
        subtitle={t('appearance.subtitle')}
      />
      <Section
        title={t('appearance.theme')}
        description={t('appearance.themeDescription')}
      >
        <ThemeSelector />
      </Section>
    </div>
  );
}
