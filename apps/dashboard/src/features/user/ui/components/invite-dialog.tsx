import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { z } from 'zod';
import { copyToClipboard } from '@/shared/lib/clipboard';
import { Choice } from '@/shared/ui/components/choice';
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
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('invite.title')}</DialogTitle>
          <DialogDescription>{t('invite.description')}</DialogDescription>
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
            <DialogFooter>
              <Button type="submit" disabled={form.formState.isSubmitting}>
                {t('invite.submit')}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
