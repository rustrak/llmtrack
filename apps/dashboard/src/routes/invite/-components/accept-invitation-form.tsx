import { zodResolver } from '@hookform/resolvers/zod';
import { Button, Field, FieldLabel, Input } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import { useForm } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { acceptInvitation } from '@/features/user/api/mutations';
import { intl } from '@/shared/i18n/intl';
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
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { AuthHeading } from '../../-components/auth-screen';

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
  const submitting = form.formState.isSubmitting;

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
    <div className="flex flex-col gap-6">
      <AuthHeading title={t('title')}>{t('subtitle')}</AuthHeading>
      <Form {...form}>
        <form onSubmit={onSubmit} className="flex flex-col gap-4" noValidate>
          <Field>
            <FieldLabel>{t('email')}</FieldLabel>
            <Input value={email} disabled readOnly />
          </Field>
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
            variant="primary"
            size="lg"
            className="mt-2 w-full"
            loading={submitting}
          >
            {t('submit')}
          </Button>
        </form>
      </Form>
    </div>
  );
}
