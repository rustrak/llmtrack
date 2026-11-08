import { zodResolver } from '@hookform/resolvers/zod';
import { Button, Input } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useForm } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { login } from '@/features/user/api/mutations';
import { returnPath } from '@/shared/api/return-path';
import { intl } from '@/shared/i18n/intl';
import { SERVER_ERROR_PATH } from '@/shared/lib/form-errors';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  FormRootError,
} from '@/shared/ui/components/form';
import { PasswordInput } from '@/shared/ui/components/password-input';
import { AuthHeading } from './auth-screen';

export function LoginForm() {
  const t = useTranslations('auth');
  const router = useRouter();
  const schema = z.object({
    email: z.email(t('form.emailInvalid')),
    password: z.string().min(1, t('form.passwordRequired')),
  });
  const form = useForm<z.infer<typeof schema>>({
    resolver: zodResolver(schema),
    defaultValues: { email: '', password: '' },
  });
  const submitting = form.formState.isSubmitting;

  const onSubmit = form.handleSubmit(async ({ email, password }) => {
    const result = await login(email, password);
    if (result.success) {
      // The account may carry a language and zone of its own.
      await intl.reload();
      // A page that sent the reader here (the consent screen) gets them back.
      const back = returnPath.take();
      if (back) router.history.push(back);
      else await router.navigate({ to: '/keys' });
      return;
    }
    // One sentence for every credential failure, on purpose: telling
    // "unknown email" apart from "wrong password" turns the form into a way
    // to find out who has an account.
    if (result.error.kind === 'unauthenticated') {
      form.setError('password', { message: t('form.invalidCredentials') });
    } else {
      form.setError(SERVER_ERROR_PATH as 'root', {
        message: t('form.unavailable'),
      });
    }
  });

  return (
    <div className="flex flex-col gap-6">
      <AuthHeading title={t('title')}>{t('subtitle')}</AuthHeading>
      <Form {...form}>
        <form onSubmit={onSubmit} className="flex flex-col gap-4" noValidate>
          <FormField
            control={form.control}
            name="email"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.email')}</FormLabel>
                <FormControl>
                  <Input
                    type="email"
                    autoComplete="email"
                    autoFocus
                    {...field}
                  />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="password"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.password')}</FormLabel>
                <FormControl>
                  <PasswordInput autoComplete="current-password" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          {/* Where an outage lands, kept off the credential fields. */}
          <FormRootError />
          <Button
            type="submit"
            variant="primary"
            size="lg"
            className="mt-2 w-full"
            loading={submitting}
          >
            {submitting ? t('form.submitting') : t('form.submit')}
          </Button>
        </form>
      </Form>
    </div>
  );
}
