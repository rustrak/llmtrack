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
import { copyToClipboard } from '@/shared/lib/clipboard';
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
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { createInvitation } from '../../api/mutations';
import { inviteLink } from '../../model/invitation';

/**
 * Invites an email, as Rustrak does: no email is sent; the link is copied
 * for the admin to share, and the invitee picks their own password.
 */
export function InviteDialog({
  onClose,
  onSaved,
}: {
  onClose: () => void;
  onSaved: () => void;
}) {
  const t = useTranslations('users');
  const tCommon = useTranslations('common');
  const tRoles = useTranslations('roles');
  const schema = z.object({
    email: z.email(t('form.emailInvalid')),
    role: z.enum(['member', 'admin']),
  });
  const form = useForm<z.infer<typeof schema>>({
    resolver: zodResolver(schema),
    defaultValues: { email: '', role: 'member' },
  });
  const serverErrors = useServerErrors(form);

  const onSubmit = form.handleSubmit(async (values) => {
    const result = await createInvitation(values);
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    const link = inviteLink(window.location.origin, result.data.token);
    const copied = await copyToClipboard(link);
    toast.success(t('invite.created', { email: result.data.email }), {
      description: copied ? t('invite.linkCopied') : link,
    });
    onSaved();
    onClose();
  });

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} size="md">
      <DialogHeader
        title={t('invite.title')}
        description={t('invite.description')}
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
              name="role"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>{t('form.role')}</FormLabel>
                  <Choice
                    aria-label={t('form.role')}
                    value={field.value}
                    onChange={field.onChange}
                    options={[
                      { value: 'member', label: tRoles('member') },
                      { value: 'admin', label: tRoles('admin') },
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
              {t('invite.submit')}
            </Button>
          </DialogFooter>
        </form>
      </Form>
    </Dialog>
  );
}
