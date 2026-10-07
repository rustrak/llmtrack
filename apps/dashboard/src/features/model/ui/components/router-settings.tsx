import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from '@tanstack/react-router';
import { Plus, X } from 'lucide-react';
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
import { Caption } from '@/shared/ui/components/page';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  FormRootError,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
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
        <FormItem>
          <FormLabel>{t(`router.${name}`)}</FormLabel>
          <FormControl>
            <Input inputMode="numeric" className="w-32" {...field} />
          </FormControl>
          <FormDescription>{t(`router.${name}Hint`)}</FormDescription>
          <FormMessage />
        </FormItem>
      )}
    />
  );

  return (
    <Form {...form}>
      <form onSubmit={onSubmit} className="max-w-3xl space-y-8" noValidate>
        <section className="space-y-4">
          <Caption>{t('router.reliability')}</Caption>
          <div className="grid gap-6 sm:grid-cols-3">
            {number('num_retries')}
            {number('allowed_fails')}
            {number('cooldown_time')}
          </div>
        </section>
        {FALLBACK_KINDS.map((kind) => (
          <FallbackEditor
            key={kind}
            kind={kind}
            control={form.control}
            names={names}
          />
        ))}
        <FormRootError />
        <Button type="submit" disabled={form.formState.isSubmitting}>
          {t('router.save')}
        </Button>
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
    <section className="space-y-3">
      <div className="space-y-1">
        <Caption>{t(`router.${kind}`)}</Caption>
        <p className="text-sm text-muted-foreground">
          {t(`router.${kind}Hint`)}
        </p>
      </div>
      {fields.map((row, index) => (
        <div
          key={row.id}
          className="flex flex-wrap items-center gap-2 rounded-lg border p-3"
        >
          <FallbackRowEditor
            control={control}
            kind={kind}
            index={index}
            names={names}
          />
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="ml-auto"
            aria-label={t('router.removeRule')}
            onClick={() => remove(index)}
          >
            <X />
          </Button>
        </div>
      ))}
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => append({ model: '', fallbacks: [] })}
      >
        <Plus />
        {t('router.addRule')}
      </Button>
    </section>
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
        className="w-48"
        value={source.field.value}
        onChange={source.field.onChange}
        placeholder={t('router.whenModel')}
        options={[
          { value: ANY, label: t('router.anyModel') },
          ...names.map((n) => ({ value: n, label: n })),
        ]}
      />
      <span className="text-sm text-muted-foreground">→</span>
      {chosen.map((name, i) => (
        <Badge key={name} variant="outline" className="gap-1 font-mono">
          {i + 1}. {name}
          <button
            type="button"
            aria-label={t('router.removeFallback', { name })}
            onClick={() =>
              targets.field.onChange(chosen.filter((n) => n !== name))
            }
          >
            <X className="size-3" />
          </button>
        </Badge>
      ))}
      {remaining.length > 0 && (
        <Choice
          aria-label={t('router.addFallback')}
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
