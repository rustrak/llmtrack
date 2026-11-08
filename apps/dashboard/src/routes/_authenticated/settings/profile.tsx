import { Tag } from '@rustrak/ui';
import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { PasswordDialog } from '@/features/user/ui/components/password-form';
import { ProfileForm } from '@/features/user/ui/components/profile-form';
import { translator } from '@/shared/i18n/intl';
import { SettingRow, SettingSection } from '@/shared/ui/components/setting-row';
import { SettingsHeader } from '@/shared/ui/components/settings-header';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';

export const Route = createFileRoute('/_authenticated/settings/profile')({
  head: () => ({ meta: [{ title: translator('settings')('profile.meta') }] }),
  component: ProfilePage,
});

/** Who the reader is here, and how they sign in. */
function ProfilePage() {
  const t = useTranslations('settings');
  const tRoles = useTranslations('roles');
  const user = useCurrentUser();
  return (
    <>
      <SettingsHeader
        title={t('profile.title')}
        description={t('profile.subtitle')}
      />
      <SettingSection
        title={t('profile.account')}
        description={t('profile.accountDescription')}
      >
        <SettingRow title={t('profile.name')} htmlFor="profile-name">
          <ProfileForm key={user.name ?? ''} user={user} />
        </SettingRow>
        <SettingRow
          title={t('profile.email')}
          description={t('profile.emailHint')}
        >
          <p className="truncate text-value text-fg">{user.email}</p>
        </SettingRow>
        <SettingRow title={t('profile.role')}>
          <Tag
            variant="soft"
            tone={user.role === 'admin' ? 'brand' : 'neutral'}
          >
            {tRoles(user.role)}
          </Tag>
        </SettingRow>
      </SettingSection>
      <SettingSection title={t('password.title')}>
        <SettingRow
          title={t('password.title')}
          description={t('password.description')}
        >
          <PasswordDialog />
        </SettingRow>
      </SettingSection>
    </>
  );
}
