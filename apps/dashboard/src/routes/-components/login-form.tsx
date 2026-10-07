import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from '@tanstack/react-router';
import { useForm } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { login } from '@/features/user/api/mutations';
import { intl } from '@/shared/i18n/intl';
import { SERVER_ERROR_PATH } from '@/shared/lib/form-errors';
import { PasswordInput } from '@/shared/ui/components/password-input';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  FormRootError,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';

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

  const onSubmit = form.handleSubmit(async ({ email, password }) => {
    const result = await login(email, password);
    if (result.success) {
      // The account may carry a language and zone of its own.
      await intl.reload();
      await router.navigate({ to: '/keys' });
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
    <div className="space-y-8">
      <div className="space-y-2">
        <h1 className="text-3xl font-bold tracking-tight">{t('title')}</h1>
        <p className="text-sm text-muted-foreground">{t('subtitle')}</p>
      </div>
      <Form {...form}>
        <form onSubmit={onSubmit} className="space-y-5" noValidate>
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
          <FormRootError />
          <Button
            type="submit"
            size="lg"
            className="w-full"
            disabled={form.formState.isSubmitting}
          >
            {form.formState.isSubmitting
              ? t('form.submitting')
              : t('form.submit')}
          </Button>
        </form>
      </Form>
    </div>
  );
}
