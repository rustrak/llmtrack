import { zodResolver } from '@hookform/resolvers/zod';
import { Tag } from 'lucide-react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { LABEL_COLORS, type Label } from '@/shared/api/schemas';
import { cn } from '@/shared/lib/utils';
import { LabelBadge, LabelDot } from '@/shared/ui/components/label-badge';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Textarea } from '@/shared/ui/components/shadcn/textarea';
import {
  ModEnterHint,
  PanelBody,
  PanelFooter,
  SidePanel,
  submitOnModEnter,
} from '@/shared/ui/components/side-panel';
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { createLabel, updateLabel } from '../../api/mutations';
import {
  type LabelValues,
  labelDefaults,
  labelSchema,
  toLabelPayload,
} from '../../model/label-form';

interface LabelPanelProps {
  /** The label to edit; none creates one. */
  label?: Label;
  onClose: () => void;
  onSaved: () => void;
}

/**
 * Creating or editing a label. The form mounts with each opening, so it
 * starts from the label as it is now.
 */
export function LabelPanel({
  open,
  ...props
}: LabelPanelProps & { open: boolean }) {
  const t = useTranslations('labels');
  return (
    <SidePanel
      open={open}
      onOpenChange={(next) => !next && props.onClose()}
      icon={Tag}
      title={
        props.label
          ? t('edit.title', { name: props.label.name })
          : t('create.title')
      }
      description={t('create.description')}
    >
      <LabelForm {...props} />
    </SidePanel>
  );
}

function LabelForm({ label, onClose, onSaved }: LabelPanelProps) {
  const t = useTranslations('labels');
  const tCommon = useTranslations('common');
  const form = useForm<LabelValues>({
    resolver: zodResolver(labelSchema(t)),
    defaultValues: labelDefaults(label),
  });
  const serverErrors = useServerErrors(form, { toast: true });
  const [name, color] = form.watch(['name', 'color']);

  const onSubmit = form.handleSubmit(async (values) => {
    const payload = toLabelPayload(values);
    const result = label
      ? await updateLabel(label.id, payload)
      : await createLabel(payload);
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    toast.success(
      t(label ? 'edit.saved' : 'create.created', { name: result.data.name }),
    );
    onSaved();
    onClose();
  });

  return (
    <Form {...form}>
      <form
        onSubmit={onSubmit}
        onKeyDown={submitOnModEnter}
        className="flex min-h-0 flex-1 flex-col"
        noValidate
      >
        <PanelBody>
          <FormField
            control={form.control}
            name="name"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.name')}</FormLabel>
                <FormControl>
                  <Input
                    autoFocus
                    placeholder={t('form.namePlaceholder')}
                    {...field}
                  />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="color"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.color')}</FormLabel>
                <fieldset className="flex flex-wrap gap-1.5">
                  <legend className="sr-only">{t('form.color')}</legend>
                  {LABEL_COLORS.map((value) => (
                    <label
                      key={value}
                      title={t(`colors.${value}`)}
                      className={cn(
                        'flex size-8 cursor-pointer items-center justify-center rounded-md border transition-colors hover:bg-muted has-focus-visible:ring-2 has-focus-visible:ring-ring',
                        field.value === value && 'border-foreground bg-muted',
                      )}
                    >
                      <input
                        type="radio"
                        name="label-color"
                        value={value}
                        checked={field.value === value}
                        onChange={() => field.onChange(value)}
                        aria-label={t(`colors.${value}`)}
                        className="sr-only"
                      />
                      <LabelDot color={value} className="size-3.5" />
                    </label>
                  ))}
                </fieldset>
                <FormDescription className="flex items-center gap-2">
                  {t('form.preview')}
                  <LabelBadge
                    label={{ name: name.trim() || t('form.name'), color }}
                  />
                </FormDescription>
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="description"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.description')}</FormLabel>
                <FormControl>
                  <Textarea rows={3} {...field} />
                </FormControl>
                <FormDescription>{t('form.descriptionHint')}</FormDescription>
                <FormMessage />
              </FormItem>
            )}
          />
        </PanelBody>
        <PanelFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {tCommon('cancel')}
          </Button>
          <Button type="submit" disabled={form.formState.isSubmitting}>
            {label ? t('edit.submit') : t('create.submit')}
            <ModEnterHint />
          </Button>
        </PanelFooter>
      </form>
    </Form>
  );
}
