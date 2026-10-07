import { createFileRoute, redirect } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { getSettings } from '@/features/settings/api/queries';
import { GeneralForm } from '@/features/settings/ui/components/general-form';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { Section, SettingsHeader } from './-components/section';

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
    <div>
      <SettingsHeader
        title={t('general.title')}
        subtitle={t('general.subtitle')}
      />
      <Section title={t('general.gateway')}>
        <GeneralForm settings={settings} />
      </Section>
    </div>
  );
}
