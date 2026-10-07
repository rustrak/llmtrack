import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { testModel } from '../../api/mutations';

/**
 * "Test connection": one token to the provider, the outcome as a
 * toast titled with the model's name. Used by the table's action and the
 * model panel alike.
 */
export function useConnectionTest() {
  const t = useTranslations('models');
  return async (name: string, request: Parameters<typeof testModel>[0]) => {
    const probe = await testModel(request);
    if (!probe.success) toast.error(readableMessage(probe.error));
    else if (probe.data.ok)
      toast.success(name, {
        description: t('test.ok', { ms: probe.data.latency_ms }),
      });
    else
      toast.error(name, {
        description: t('test.failed', {
          status: probe.data.status ?? '—',
          message: probe.data.message,
        }),
      });
  };
}
