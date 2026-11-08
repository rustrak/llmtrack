import { zodResolver } from '@hookform/resolvers/zod';
import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
} from '@rustrak/ui';
import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
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
import { changePassword } from '../../api/mutations';

const EMPTY = { current_password: '', new_password: '', confirm: '' };

const FIELDS = [
  ['current_password', 'current', 'current-password'],
  ['new_password', 'new', 'new-password'],
  ['confirm', 'confirm', 'new-password'],
] as const;

/** The "Change password" button and the dialog it opens, as in Rustrak. */
export function PasswordDialog() {
  const t = useTranslations('settings');
  const tCommon = useTranslations('common');
  const [open, setOpen] = useState(false);
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
    defaultValues: EMPTY,
  });
  const serverErrors = useServerErrors(form);
  const pending = form.formState.isSubmitting;

  // Typed passwords must not survive the dialog closing, whichever way.
  const onOpenChange = (next: boolean) => {
    if (next) return setOpen(true);
    if (pending) return;
    form.reset(EMPTY);
    setOpen(false);
  };

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
    form.reset(EMPTY);
    setOpen(false);
  });

  return (
    <>
      <Button onClick={() => setOpen(true)}>{t('password.submit')}</Button>
      <Dialog open={open} onOpenChange={onOpenChange} size="md">
        <DialogHeader
          title={t('password.submit')}
          description={t('password.description')}
        />
        <Form {...form}>
          <form
            noValidate
            onSubmit={onSubmit}
            className="flex min-h-0 flex-col"
          >
            <DialogBody className="flex flex-col gap-4">
              {FIELDS.map(([name, label, autoComplete]) => (
                <FormField
                  key={name}
                  control={form.control}
                  name={name}
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t(`password.${label}`)}</FormLabel>
                      <FormControl>
                        <PasswordInput
                          autoComplete={autoComplete}
                          disabled={pending}
                          {...field}
                        />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />
              ))}
              <FormRootError />
            </DialogBody>
            <DialogFooter>
              <DialogClose
                disabled={pending}
                render={
                  <Button variant="secondary">{tCommon('cancel')}</Button>
                }
              />
              <Button type="submit" variant="primary" loading={pending}>
                {t('password.submit')}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </Dialog>
    </>
  );
}
