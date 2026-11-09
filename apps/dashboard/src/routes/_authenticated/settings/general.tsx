import { createFileRoute, redirect } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { getSettings } from '@/features/settings/api/queries';
import { GeneralForm } from '@/features/settings/ui/components/general-form';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { SettingSection } from '@/shared/ui/components/setting-row';
import { SettingsHeader } from '@/shared/ui/components/settings-header';

export const Route = createFileRoute('/_authenticated/settings/general')({
  beforeLoad: ({ context }) => {
    if (context.state !== 'authenticated' || context.user.role !== 'admin') {
      throw redirect({ to: '/settings/profile' });
    }
  },
  loader: async () => ({ settings: unwrap(await getSettings()) }),
  head: () => ({ meta: [{ title: translator('settings')('general.meta') }] }),
  component: GeneralPage,
});

function GeneralPage() {
  const t = useTranslations('settings');
  const { settings } = Route.useLoaderData();
  return (
    <>
      <SettingsHeader
        title={t('general.title')}
        description={t('general.subtitle')}
      />
      <SettingSection title={t('general.gateway')}>
        <GeneralForm settings={settings} />
      </SettingSection>
    </>
  );
}
