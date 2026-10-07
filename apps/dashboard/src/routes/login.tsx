import { createFileRoute, redirect } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { session } from '@/shared/api/session';
import { translator } from '@/shared/i18n/intl';
import { LlmtrackWordmark } from '@/shared/ui/components/llmtrack-wordmark';
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
  const t = useTranslations('auth');
  const stats = [
    { value: '<1ms', label: t('statOverhead') },
    { value: '1', label: t('statEndpoint') },
    { value: '$', label: t('statSpend') },
  ];
  return (
    <div className="min-h-screen flex">
      <div className="hidden lg:flex lg:w-1/2 bg-background flex-col justify-between p-12 relative overflow-hidden">
        <div className="absolute bottom-0 left-0 right-0 h-32 bg-gradient-to-t from-background to-transparent z-10" />
        <LlmtrackWordmark className="relative z-20 text-2xl" />
        <div className="relative z-20 max-w-xl">
          <h2 className="text-6xl xl:text-7xl font-extrabold tracking-tighter leading-[1.05] mb-8">
            {t('heroTitle')}
            <span className="text-primary">.</span>
          </h2>
          <p className="text-muted-foreground text-lg font-medium leading-relaxed max-w-md">
            {t('heroDescription')}
          </p>
          <div className="mt-12 flex items-center gap-8">
            {stats.map((stat) => (
              <div key={stat.label}>
                <span className="text-2xl font-bold text-primary">
                  {stat.value}
                </span>
                <p className="text-sm text-muted-foreground">{stat.label}</p>
              </div>
            ))}
          </div>
        </div>
        <p className="relative z-20 text-xs text-muted-foreground font-mono">
          &copy; {new Date().getFullYear()} llmtrack
        </p>
      </div>
      <div className="w-full lg:w-1/2 bg-card flex items-center justify-center p-8 lg:p-12">
        <div className="w-full max-w-[420px] space-y-10">
          <LlmtrackWordmark className="lg:hidden text-2xl" />
          <LoginForm />
        </div>
      </div>
    </div>
  );
}
