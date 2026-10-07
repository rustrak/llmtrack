import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { PasswordForm } from '@/features/user/ui/components/password-form';
import { ProfileForm } from '@/features/user/ui/components/profile-form';
import { RegionalSettings } from '@/features/user/ui/components/regional-settings';
import { translator } from '@/shared/i18n/intl';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { Section, SettingsHeader } from './-components/section';

export const Route = createFileRoute('/_authenticated/settings/profile')({
  head: () => ({ meta: [{ title: translator('settings')('profile.meta') }] }),
  component: ProfilePage,
});

function ProfilePage() {
  const t = useTranslations('settings');
  const tRoles = useTranslations('roles');
  const user = useCurrentUser();
  return (
    <div>
      <SettingsHeader
        title={t('profile.title')}
        subtitle={t('profile.subtitle')}
      />
      <Section
        title={t('profile.account')}
        description={t('profile.accountDescription')}
      >
        <div className="flex items-center gap-2 text-sm">
          <span className="text-muted-foreground">{t('profile.role')}</span>
          <Badge variant="outline">{tRoles(user.role)}</Badge>
        </div>
        <ProfileForm key={user.name ?? ''} user={user} />
      </Section>
      <Section
        title={t('password.title')}
        description={t('password.description')}
      >
        <PasswordForm />
      </Section>
      <Section
        title={t('profile.regional')}
        description={t('profile.regionalDescription')}
      >
        <RegionalSettings user={user} />
      </Section>
    </div>
  );
}
