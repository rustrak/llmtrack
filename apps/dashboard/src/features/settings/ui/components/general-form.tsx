import { Button, Input } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Settings } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { SettingRow } from '@/shared/ui/components/setting-row';
import { updateSettings } from '../../api/mutations';

/** The public URL row: where applications reach the gateway. */
export function GeneralForm({ settings }: { settings: Settings }) {
  const t = useTranslations('settings');
  const router = useRouter();
  const [publicUrl, setPublicUrl] = useState(settings.public_url ?? '');
  const [pending, setPending] = useState(false);
  const changed = publicUrl.trim() !== (settings.public_url ?? '');

  return (
    <SettingRow
      title={t('general.publicUrl')}
      description={t('general.publicUrlHint')}
      htmlFor="public-url"
      wide
    >
      <form
        className="flex items-center gap-2"
        onSubmit={async (event) => {
          event.preventDefault();
          setPending(true);
          const result = await updateSettings({
            public_url: publicUrl.trim() || null,
          });
          setPending(false);
          if (!result.success) {
            toast.error(readableMessage(result.error));
            return;
          }
          toast.success(t('general.saved'));
          await router.invalidate();
        }}
      >
        <Input
          id="public-url"
          boxClassName="min-w-0 flex-1"
          className="font-mono"
          placeholder="https://llm.example.com"
          value={publicUrl}
          onChange={(event) => setPublicUrl(event.target.value)}
        />
        <Button
          type="submit"
          variant="primary"
          disabled={!changed}
          loading={pending}
        >
          {t('general.save')}
        </Button>
      </form>
    </SettingRow>
  );
}
