import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Settings } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import { updateSettings } from '../../api/mutations';

export function GeneralForm({ settings }: { settings: Settings }) {
  const t = useTranslations('settings');
  const router = useRouter();
  const [publicUrl, setPublicUrl] = useState(settings.public_url ?? '');
  const [pending, setPending] = useState(false);

  return (
    <form
      className="max-w-lg space-y-6"
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
      <div className="space-y-1.5">
        <Label htmlFor="public-url">{t('general.publicUrl')}</Label>
        <Input
          id="public-url"
          className="font-mono"
          placeholder="https://llm.example.com"
          value={publicUrl}
          onChange={(event) => setPublicUrl(event.target.value)}
        />
        <p className="text-xs text-muted-foreground">
          {t('general.publicUrlHint')}
        </p>
      </div>
      <Button type="submit" disabled={pending}>
        {t('general.save')}
      </Button>
    </form>
  );
}
