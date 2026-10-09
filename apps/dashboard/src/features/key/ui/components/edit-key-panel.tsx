import { zodResolver } from '@hookform/resolvers/zod';
import { KeyRound } from 'lucide-react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { ApiKey, LabelRef, ModelRef } from '@/shared/api/schemas';
import { LimitSections } from '@/shared/ui/components/limit-sections';
import { ModelPicker } from '@/shared/ui/components/model-picker';
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
import {
  ModEnterHint,
  PanelBody,
  PanelFooter,
  PanelSection,
  SidePanel,
  submitOnModEnter,
} from '@/shared/ui/components/side-panel';
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { updateKey } from '../../api/mutations';
import {
  editDefaults,
  toUpdateKeyPayload,
  type UpdateKeyValues,
  updateKeySchema,
} from '../../model/key-form';
import { BodySection } from './body-section';
import { LabelsField, PersonField } from './create-key-panel';

interface EditKeyProps {
  apiKey: ApiKey;
  choices: ModelRef[];
  people?: { id: number; name: string }[];
  labels: LabelRef[];
  onClose: () => void;
  onSaved: () => void;
}

/**
 * Editing a key. Mounted all along and driven by `open`, so it slides in and
 * out; the form inside mounts with each opening, so it starts from the key
 * as it is now.
 */
export function EditKeyPanel({
  open,
  ...props
}: EditKeyProps & { open: boolean }) {
  const t = useTranslations('keys');
  const { apiKey, onClose } = props;
  return (
    <SidePanel
      open={open}
      onOpenChange={(next) => !next && onClose()}
      icon={KeyRound}
      title={t('edit.title', { name: apiKey.name })}
      description={[apiKey.key_hint, apiKey.team_name ?? t('personal')].join(
        ' · ',
      )}
    >
      <EditKeyForm {...props} />
    </SidePanel>
  );
}

function EditKeyForm({
  apiKey,
  choices,
  people,
  labels,
  onClose,
  onSaved,
}: EditKeyProps) {
  const t = useTranslations('keys');
  const tCommon = useTranslations('common');
  const money = useMoney();
  const defaults = editDefaults(apiKey, money.perUsd);
  const form = useForm<UpdateKeyValues>({
    resolver: zodResolver(updateKeySchema(t)),
    defaultValues: defaults,
  });
  const serverErrors = useServerErrors(form, { toast: true });
  const expiresOn = form.watch('expires_on');

  const onSubmit = form.handleSubmit(async (values) => {
    const result = await updateKey(
      apiKey.id,
      toUpdateKeyPayload(values, defaults.expires_on, money.perUsd),
    );
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    toast.success(t('edit.saved', { name: values.name }));
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
                  <Input {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <PersonField people={people} />
          <FormField
            control={form.control}
            name="models"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.models')}</FormLabel>
                <ModelPicker
                  models={choices}
                  value={field.value}
                  onChange={field.onChange}
                  emptyMeaning={
                    apiKey.team_id === null
                      ? t('form.modelsEmptyPersonal')
                      : t('form.modelsEmpty')
                  }
                />
              </FormItem>
            )}
          />
          <LabelsField labels={labels} />
          <div className="space-y-2">
            <LimitSections />
            <BodySection />
            <PanelSection
              title={t('form.sections.expiry')}
              summary={
                expiresOn
                  ? money.date(`${expiresOn}T12:00:00`)
                  : t('expiry.never')
              }
              forceOpen={Boolean(form.formState.errors.expires_on)}
            >
              <FormField
                control={form.control}
                name="expires_on"
                render={({ field }) => (
                  <FormItem>
                    <FormControl>
                      <Input
                        type="date"
                        aria-label={t('form.expires')}
                        {...field}
                      />
                    </FormControl>
                    <FormDescription>{t('form.expiresOnHint')}</FormDescription>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </PanelSection>
          </div>
        </PanelBody>
        <PanelFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {tCommon('cancel')}
          </Button>
          <Button type="submit" disabled={form.formState.isSubmitting}>
            {t('edit.submit')}
            <ModEnterHint />
          </Button>
        </PanelFooter>
      </form>
    </Form>
  );
}
