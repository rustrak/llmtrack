import { Button, Input } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { User } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { updateProfile } from '../../api/mutations';

/** The name field of the profile row, saved on its own button. */
export function ProfileForm({ user }: { user: User }) {
  const t = useTranslations('settings');
  const router = useRouter();
  const [name, setName] = useState(user.name ?? '');
  const [pending, setPending] = useState(false);
  const changed = name.trim() !== (user.name ?? '');

  return (
    <form
      className="flex items-center gap-2"
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
      <Input
        id="profile-name"
        boxClassName="min-w-0 flex-1"
        value={name}
        maxLength={100}
        placeholder={t('profile.namePlaceholder')}
        onChange={(event) => setName(event.target.value)}
      />
      <Button
        type="submit"
        variant="primary"
        disabled={!changed}
        loading={pending}
      >
        {t('profile.save')}
      </Button>
    </form>
  );
}
