import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { listKeys } from '@/features/key/api/queries';
import { Playground } from '@/features/playground/ui/components/playground';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { MAX_PAGE_SIZE } from '@/shared/lib/list-params';
import { Page } from '@/shared/ui/components/page';

export const Route = createFileRoute('/_authenticated/playground')({
  loader: async () => {
    // ponytail: the first 200 active keys; search them if there are more.
    const keys = await listKeys({ per_page: MAX_PAGE_SIZE, status: 'active' });
    return { keys: unwrap(keys).data };
  },
  head: () => ({ meta: [{ title: translator('playground')('meta.title') }] }),
  component: PlaygroundPage,
});

function PlaygroundPage() {
  const t = useTranslations('playground');
  const { keys } = Route.useLoaderData();
  return (
    <Page fill title={t('title')} description={t('subtitle')}>
      <Playground keys={keys} />
    </Page>
  );
}
