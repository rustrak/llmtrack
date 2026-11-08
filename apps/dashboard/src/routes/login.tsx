import { createFileRoute, redirect } from '@tanstack/react-router';
import { session } from '@/shared/api/session';
import { translator } from '@/shared/i18n/intl';
import { AuthScreen } from './-components/auth-screen';
import { LoginForm } from './-components/login-form';

export const Route = createFileRoute('/login')({
  beforeLoad: async () => {
    if ((await session.ensure()).state === 'authenticated') {
      throw redirect({ to: '/keys' });
    }
  },
  head: () => {
    const t = translator('auth');
    return { meta: [{ title: t('meta.title') }] };
  },
  component: LoginPage,
});

function LoginPage() {
  return (
    <AuthScreen>
      <LoginForm />
    </AuthScreen>
  );
}
