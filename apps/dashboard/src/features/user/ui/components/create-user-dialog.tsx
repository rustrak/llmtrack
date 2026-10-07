import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { Choice } from '@/shared/ui/components/choice';
import { PasswordInput } from '@/shared/ui/components/password-input';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/shared/ui/components/shadcn/dialog';
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
import { createUser } from '../../api/mutations';

export function CreateUserDialog({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: () => void;
}) {
  const t = useTranslations('users');
  const schema = z.object({
    email: z.email(t('form.emailInvalid')),
    password: z.string().min(8, t('form.passwordShort')),
    role: z.enum(['member', 'admin']),
  });
  const form = useForm<z.infer<typeof schema>>({
    resolver: zodResolver(schema),
    defaultValues: { email: '', password: '', role: 'member' },
  });

  const serverErrors = useServerErrors(form);

  const onSubmit = form.handleSubmit(async (values) => {
    const result = await createUser(values);
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    toast.success(t('create.done', { email: result.data.email }));
    onSaved();
    onClose();
  });

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('create.title')}</DialogTitle>
          <DialogDescription>{t('create.description')}</DialogDescription>
        </DialogHeader>
        <Form {...form}>
          <form onSubmit={onSubmit} className="space-y-4" noValidate>
            <FormField
              control={form.control}
              name="email"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>{t('form.email')}</FormLabel>
                  <FormControl>
                    <Input type="email" autoComplete="off" {...field} />
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
                    <PasswordInput autoComplete="new-password" {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            <FormField
              control={form.control}
              name="role"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>{t('form.role')}</FormLabel>
                  <Choice
                    aria-label={t('form.role')}
                    value={field.value}
                    onChange={field.onChange}
                    options={[
                      { value: 'member', label: t('form.roleMember') },
                      { value: 'admin', label: t('form.roleAdmin') },
                    ]}
                  />
                </FormItem>
              )}
            />
            <FormRootError />
            <DialogFooter>
              <Button type="submit" disabled={form.formState.isSubmitting}>
                {t('create.submit')}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
