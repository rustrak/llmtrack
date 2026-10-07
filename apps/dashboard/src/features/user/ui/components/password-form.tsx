import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
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
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { changePassword } from '../../api/mutations';

export function PasswordForm() {
  const t = useTranslations('settings');
  const schema = z
    .object({
      current_password: z.string().min(1, t('password.currentRequired')),
      new_password: z.string().min(8, t('password.tooShort')),
      confirm: z.string(),
    })
    .refine((v) => v.new_password === v.confirm, {
      path: ['confirm'],
      message: t('password.mismatch'),
    });
  const form = useForm<z.infer<typeof schema>>({
    resolver: zodResolver(schema),
    defaultValues: { current_password: '', new_password: '', confirm: '' },
  });
  const serverErrors = useServerErrors(form);

  const onSubmit = form.handleSubmit(async (values) => {
    const result = await changePassword(
      values.current_password,
      values.new_password,
    );
    if (!result.success) {
      if (result.error.kind === 'unauthenticated') {
        form.setError('current_password', {
          message: t('password.wrongCurrent'),
        });
      } else {
        serverErrors(result.error);
      }
      return;
    }
    toast.success(t('password.changed'));
    form.reset();
  });

  return (
    <Form {...form}>
      <form onSubmit={onSubmit} className="max-w-sm space-y-4" noValidate>
        <FormField
          control={form.control}
          name="current_password"
          render={({ field }) => (
            <FormItem>
              <FormLabel>{t('password.current')}</FormLabel>
              <FormControl>
                <PasswordInput autoComplete="current-password" {...field} />
              </FormControl>
              <FormMessage />
            </FormItem>
          )}
        />
        <FormField
          control={form.control}
          name="new_password"
          render={({ field }) => (
            <FormItem>
              <FormLabel>{t('password.new')}</FormLabel>
              <FormControl>
                <PasswordInput autoComplete="new-password" {...field} />
              </FormControl>
              <FormMessage />
            </FormItem>
          )}
        />
        <FormField
          control={form.control}
          name="confirm"
          render={({ field }) => (
            <FormItem>
              <FormLabel>{t('password.confirm')}</FormLabel>
              <FormControl>
                <PasswordInput autoComplete="new-password" {...field} />
              </FormControl>
              <FormMessage />
            </FormItem>
          )}
        />
        <FormRootError />
        <Button type="submit" disabled={form.formState.isSubmitting}>
          {t('password.submit')}
        </Button>
      </form>
    </Form>
  );
}
