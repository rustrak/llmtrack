import { createFileRoute, redirect } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { getCatalogStatus } from '@/features/model/api/queries';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { CatalogPanel } from './-components/catalog-panel';
import { SettingsHeader } from './-components/section';

export const Route = createFileRoute('/_authenticated/settings/pricing')({
  beforeLoad: ({ context }) => {
    if (context.state !== 'authenticated' || context.user.role !== 'admin') {
      throw redirect({ to: '/settings/profile' });
    }
  },
  loader: async () => ({ status: unwrap(await getCatalogStatus()) }),
  head: () => ({ meta: [{ title: translator('settings')('pricing.meta') }] }),
  component: PricingPage,
});

function PricingPage() {
  const t = useTranslations('settings');
  const { status } = Route.useLoaderData();
  return (
    <div>
      <SettingsHeader
        title={t('pricing.title')}
        subtitle={t('pricing.subtitle')}
      />
      <CatalogPanel key={status.synced_at ?? 'builtin'} status={status} />
    </div>
  );
}
