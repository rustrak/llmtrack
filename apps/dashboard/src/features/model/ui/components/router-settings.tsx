import { zodResolver } from '@hookform/resolvers/zod';
import { Button, CloseIcon, Input, NewIcon, Tooltip } from '@rustrak/ui';
import { useRouter } from '@tanstack/react-router';
import {
  type Control,
  useController,
  useFieldArray,
  useForm,
} from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { RouterSettings } from '@/shared/api/schemas';
import { Choice } from '@/shared/ui/components/choice';
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormMessage,
  FormRootError,
} from '@/shared/ui/components/form';
import { SettingRow, SettingSection } from '@/shared/ui/components/setting-row';
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { saveRouterSettings } from '../../api/mutations';
import {
  FALLBACK_KINDS,
  type FallbackKind,
  type RouterFormValues,
  routerDefaults,
  routerFormSchema,
  toRouterPayload,
} from '../../model/router-form';

const ANY = '*';

/**
 * The router settings: how often a failed request is retried, when a
 * deployment sits out, and which models take over when one cannot answer.
 */
export function RouterSettingsForm({
  settings,
  names,
}: {
  settings: RouterSettings;
  /** Every model name, once. */
  names: string[];
}) {
  const t = useTranslations('models');
  const router = useRouter();
  const form = useForm<RouterFormValues>({
    resolver: zodResolver(routerFormSchema(t)),
    defaultValues: routerDefaults(settings),
  });
  const serverErrors = useServerErrors(form);

  const onSubmit = form.handleSubmit(async (values) => {
    const result = await saveRouterSettings(toRouterPayload(values));
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    toast.success(t('router.saved'));
    form.reset(routerDefaults(result.data));
    await router.invalidate();
  });

  const number = (name: 'num_retries' | 'allowed_fails' | 'cooldown_time') => (
    <FormField
      control={form.control}
      name={name}
      render={({ field }) => (
        <SettingRow
          title={t(`router.${name}`)}
          description={t(`router.${name}Hint`)}
        >
          <FormItem>
            <FormControl>
              <Input
                inputMode="numeric"
                className="font-mono tabular-nums"
                aria-label={t(`router.${name}`)}
                {...field}
              />
            </FormControl>
            <FormMessage />
          </FormItem>
        </SettingRow>
      )}
    />
  );

  return (
    <Form {...form}>
      <form onSubmit={onSubmit} className="max-w-3xl" noValidate>
        <SettingSection title={t('router.reliability')}>
          {number('num_retries')}
          {number('allowed_fails')}
          {number('cooldown_time')}
        </SettingSection>
        {FALLBACK_KINDS.map((kind) => (
          <FallbackEditor
            key={kind}
            kind={kind}
            control={form.control}
            names={names}
          />
        ))}
        <div className="mt-4 flex items-center justify-end gap-3">
          <FormRootError />
          <Button
            type="submit"
            variant="primary"
            loading={form.formState.isSubmitting}
          >
            {t('router.save')}
          </Button>
        </div>
      </form>
    </Form>
  );
}

/** One fallback list: per model (or any model), the models to try next, in order. */
function FallbackEditor({
  kind,
  control,
  names,
}: {
  kind: FallbackKind;
  control: Control<RouterFormValues>;
  names: string[];
}) {
  const t = useTranslations('models');
  const { fields, append, remove } = useFieldArray({ control, name: kind });
  return (
    <SettingSection
      title={t(`router.${kind}`)}
      description={t(`router.${kind}Hint`)}
    >
      {fields.map((row, index) => (
        <div
          key={row.id}
          className="flex flex-wrap items-center gap-2 border-border-divider border-b px-5 py-3"
        >
          <FallbackRowEditor
            control={control}
            kind={kind}
            index={index}
            names={names}
          />
          <Tooltip content={t('router.removeRule')}>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              icon={CloseIcon}
              className="ml-auto"
              aria-label={t('router.removeRule')}
              onClick={() => remove(index)}
            />
          </Tooltip>
        </div>
      ))}
      <div className="px-5 py-2.5">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          icon={NewIcon}
          onClick={() => append({ model: '', fallbacks: [] })}
        >
          {t('router.addRule')}
        </Button>
      </div>
    </SettingSection>
  );
}

function FallbackRowEditor({
  control,
  kind,
  index,
  names,
}: {
  control: Control<RouterFormValues>;
  kind: FallbackKind;
  index: number;
  names: string[];
}) {
  const t = useTranslations('models');
  const source = useController({ control, name: `${kind}.${index}.model` });
  const targets = useController({
    control,
    name: `${kind}.${index}.fallbacks`,
  });
  const chosen = targets.field.value;
  const remaining = names.filter(
    (n) => n !== source.field.value && !chosen.includes(n),
  );
  return (
    <>
      <Choice
        aria-label={t('router.whenModel')}
        size="sm"
        className="w-48"
        value={source.field.value}
        onChange={source.field.onChange}
        placeholder={t('router.whenModel')}
        options={[
          { value: ANY, label: t('router.anyModel') },
          ...names.map((n) => ({ value: n, label: n })),
        ]}
      />
      <span className="text-fg-ghost">→</span>
      {chosen.map((name, i) => (
        <span
          key={name}
          className="inline-flex h-control-sm items-center gap-1.5 rounded-md border border-border-subtle bg-surface-chip pr-1 pl-2 font-mono text-mono-sm text-fg"
        >
          <span className="text-fg-meta">{i + 1}.</span>
          {name}
          <button
            type="button"
            aria-label={t('router.removeFallback', { name })}
            className="flex size-5 items-center justify-center rounded-sm text-fg-ghost transition-[background-color,color] duration-fast hover:bg-surface-selected hover:text-fg"
            onClick={() =>
              targets.field.onChange(chosen.filter((n) => n !== name))
            }
          >
            <CloseIcon size="xs" />
          </button>
        </span>
      ))}
      {remaining.length > 0 && (
        <Choice
          aria-label={t('router.addFallback')}
          size="sm"
          className="w-44"
          value=""
          onChange={(name) => name && targets.field.onChange([...chosen, name])}
          placeholder={t('router.addFallback')}
          options={remaining.map((n) => ({ value: n, label: n }))}
        />
      )}
    </>
  );
}
