import { zodResolver } from '@hookform/resolvers/zod';
import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
  Input,
} from '@rustrak/ui';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { Choice } from '@/shared/ui/components/choice';
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
import { createUser } from '../../api/mutations';

export function CreateUserDialog({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: () => void;
}) {
  const t = useTranslations('users');
  const tCommon = useTranslations('common');
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
    <Dialog open onOpenChange={(open) => !open && onClose()} size="md">
      <DialogHeader
        title={t('create.title')}
        description={t('create.description')}
      />
      <Form {...form}>
        <form onSubmit={onSubmit} className="flex min-h-0 flex-col" noValidate>
          <DialogBody className="flex flex-col gap-4">
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
          </DialogBody>
          <DialogFooter>
            <DialogClose
              render={<Button variant="secondary">{tCommon('cancel')}</Button>}
            />
            <Button
              type="submit"
              variant="primary"
              loading={form.formState.isSubmitting}
            >
              {t('create.submit')}
            </Button>
          </DialogFooter>
        </form>
      </Form>
    </Dialog>
  );
}
