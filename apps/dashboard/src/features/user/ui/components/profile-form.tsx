import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { User } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import { updateProfile } from '../../api/mutations';

export function ProfileForm({ user }: { user: User }) {
  const t = useTranslations('settings');
  const router = useRouter();
  const [name, setName] = useState(user.name ?? '');
  const [pending, setPending] = useState(false);
  const changed = name.trim() !== (user.name ?? '');

  return (
    <form
      className="space-y-4"
      onSubmit={async (event) => {
        event.preventDefault();
        setPending(true);
        const result = await updateProfile({ name: name.trim() || null });
        setPending(false);
        if (!result.success) {
          toast.error(readableMessage(result.error));
          return;
        }
        toast.success(t('profile.saved'));
        await router.invalidate();
      }}
    >
      <div className="space-y-1.5">
        <Label htmlFor="profile-name">{t('profile.name')}</Label>
        <Input
          id="profile-name"
          value={name}
          maxLength={100}
          placeholder={t('profile.namePlaceholder')}
          onChange={(event) => setName(event.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="profile-email">{t('profile.email')}</Label>
        <Input id="profile-email" value={user.email} disabled />
        <p className="text-xs text-muted-foreground">
          {t('profile.emailHint')}
        </p>
      </div>
      <Button type="submit" disabled={!changed || pending}>
        {t('profile.save')}
      </Button>
    </form>
  );
}
