import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from '@tanstack/react-router';
import { useForm } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { acceptInvitation } from '@/features/user/api/mutations';
import { intl } from '@/shared/i18n/intl';
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
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';

export function AcceptInvitationForm({
  token,
  email,
}: {
  token: string;
  email: string;
}) {
  const t = useTranslations('invite');
  const router = useRouter();
  const schema = z
    .object({
      password: z.string().min(8, t('passwordShort')),
      confirmPassword: z.string(),
    })
    .refine((v) => v.password === v.confirmPassword, {
      path: ['confirmPassword'],
      message: t('passwordMismatch'),
    });
  const form = useForm<z.infer<typeof schema>>({
    resolver: zodResolver(schema),
    defaultValues: { password: '', confirmPassword: '' },
  });
  const serverErrors = useServerErrors(form);

  const onSubmit = form.handleSubmit(async ({ password }) => {
    const result = await acceptInvitation(token, password);
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    await intl.reload();
    await router.navigate({ to: '/keys' });
  });

  return (
    <div className="space-y-8">
      <div className="space-y-2">
        <h1 className="text-3xl font-bold tracking-tight">{t('title')}</h1>
        <p className="text-sm text-muted-foreground">{t('subtitle')}</p>
      </div>
      <Form {...form}>
        <form onSubmit={onSubmit} className="space-y-5" noValidate>
          <div className="space-y-2">
            <label htmlFor="invite-email" className="text-sm font-medium">
              {t('email')}
            </label>
            <Input id="invite-email" value={email} disabled readOnly />
          </div>
          <FormField
            control={form.control}
            name="password"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('password')}</FormLabel>
                <FormControl>
                  <PasswordInput
                    autoComplete="new-password"
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
            name="confirmPassword"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('confirmPassword')}</FormLabel>
                <FormControl>
                  <PasswordInput autoComplete="new-password" {...field} />
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
            {t('submit')}
          </Button>
        </form>
      </Form>
    </div>
  );
}
