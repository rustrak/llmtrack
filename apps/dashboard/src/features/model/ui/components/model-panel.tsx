import { zodResolver } from '@hookform/resolvers/zod';
import { Boxes } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { type UseFormReturn, useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { CatalogEntry, Model, Provider } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
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
import { createModel, updateModel } from '../../api/mutations';
import { searchCatalog } from '../../api/queries';
import {
  defaultsFor,
  type ModelFormValues,
  type ModelPayload,
  mappingFor,
  modelFormSchema,
  toModelPayloads,
} from '../../model/model-form';
import { ConnectionFields } from './connection-fields';
import { MappingsTable } from './mappings-table';
import { PricingSection } from './pricing-section';
import { ProviderModelPicker } from './provider-model-picker';
import { useConnectionTest } from './use-connection-test';

interface ModelFormProps {
  model?: Model;
  providers: Provider[];
  onClose: () => void;
  onSaved: () => void;
}

/**
 * Adds models: a provider, its models picked from the
 * price list (or typed), their public names; then connection, pricing and
 * weight, folded with what each is set to. Editing uses the same form for
 * one model. Mounted all along and driven by `open`; the form mounts with
 * each opening, so it starts from `model` as it is.
 */
export function ModelPanel({
  open,
  ...props
}: ModelFormProps & { open: boolean }) {
  const t = useTranslations('models');
  const { model, onClose } = props;
  return (
    <SidePanel
      open={open}
      onOpenChange={(next) => !next && onClose()}
      size="lg"
      icon={Boxes}
      title={
        model
          ? t('dialog.editTitle', { name: model.name })
          : t('dialog.addTitle')
      }
      description={t('dialog.description')}
    >
      <ModelForm key={model?.id ?? 'new'} {...props} />
    </SidePanel>
  );
}

function ModelForm({ model, providers, onClose, onSaved }: ModelFormProps) {
  const t = useTranslations('models');
  const tCommon = useTranslations('common');
  const form = useForm<ModelFormValues>({
    resolver: zodResolver(modelFormSchema(t)),
    defaultValues: defaultsFor(model),
  });
  const serverErrors = useServerErrors(form, { toast: true });
  const [entries, setEntries] = useState<ReadonlyMap<string, CatalogEntry>>(
    new Map(),
  );
  const onLoaded = useCallback(
    (list: CatalogEntry[]) =>
      setEntries(
        (known) => new Map([...known, ...list.map((e) => [e.key, e] as const)]),
      ),
    [],
  );
  const addEntry = useCallback(
    (entry: CatalogEntry) =>
      setEntries((known) => new Map(known).set(entry.key, entry)),
    [],
  );
  // The entry a model is billed by may not be in its provider's list
  // (an OpenAI-compatible server matched by name): look it up.
  useEffect(() => {
    const key = model?.catalog_key;
    if (!key) return;
    searchCatalog(key).then((result) => {
      const hit = result.success && result.data.find((e) => e.key === key);
      if (hit) addEntry(hit);
    });
  }, [model?.catalog_key, addEntry]);
  const provider = providers.find((p) => p.id === form.watch('provider'));
  const mappings = form.watch('mappings');

  const toggle = (entry: CatalogEntry) => {
    const current = form.getValues('mappings');
    if (model) {
      form.setValue('mappings', [
        {
          ...mappingFor(entry),
          public_name: current[0]?.public_name ?? entry.model,
        },
      ]);
    } else if (current.some((m) => m.catalog_key === entry.key)) {
      form.setValue(
        'mappings',
        current.filter((m) => m.catalog_key !== entry.key),
      );
    } else {
      form.setValue('mappings', [...current, mappingFor(entry)]);
    }
    form.clearErrors('mappings');
  };
  const addCustom = (name: string) => {
    const row = mappingFor({ key: null, model: name });
    form.setValue(
      'mappings',
      model ? [row] : [...form.getValues('mappings'), row],
    );
    form.clearErrors('mappings');
  };

  const onSubmit = form.handleSubmit(async (values) => {
    const payloads = toModelPayloads(values, model ? 'edit' : 'create');
    const failure = await saveAll(payloads, model);
    if (failure) {
      serverErrors(failure.error);
      if (!model)
        toast.error(`${failure.name}: ${readableMessage(failure.error)}`);
      onSaved();
      return;
    }
    toast.success(
      model
        ? t('dialog.updated', { name: payloads[0].name })
        : t('dialog.createdMany', { count: payloads.length }),
    );
    onSaved();
    onClose();
  });

  const errors = form.formState.errors;
  const apiBase = form.watch('api_base').trim();
  const hasKey =
    Boolean(form.watch('api_key').trim()) ||
    (Boolean(model?.has_api_key) && !form.watch('clear_api_key'));
  const weight = form.watch('weight').trim();
  const picker = (
    <ProviderModelPicker
      provider={form.watch('provider')}
      picked={mappings.map((m) => m.catalog_key)}
      onToggle={toggle}
      onCustom={addCustom}
      onLoaded={onLoaded}
    />
  );

  return (
    <Form {...form}>
      <form
        onSubmit={onSubmit}
        onKeyDown={submitOnModEnter}
        className="flex min-h-0 flex-1 flex-col"
        noValidate
      >
        <PanelBody>
          <FormItem>
            <FormLabel>{t('form.provider')}</FormLabel>
            <Choice
              aria-label={t('form.provider')}
              value={form.watch('provider')}
              onChange={(value) => {
                form.setValue('provider', value);
                if (!model) form.setValue('mappings', []);
              }}
              options={providers.map((p) => ({
                value: p.id,
                label: p.label,
              }))}
            />
          </FormItem>
          <FormItem>
            <FormLabel>{model ? t('form.model') : t('form.models')}</FormLabel>
            {!model && picker}
            <MappingsTable
              form={form}
              entries={entries}
              removable={!model}
              onEntry={addEntry}
            />
            <FormMessage>{errors.mappings?.message}</FormMessage>
          </FormItem>
          <div className="space-y-2">
            {model && (
              <PanelSection
                title={t('form.changeModel')}
                summary={mappings[0]?.upstream_model ?? ''}
              >
                {picker}
              </PanelSection>
            )}
            <PanelSection
              title={t('dialog.connection')}
              summary={[
                apiBase ? hostOf(apiBase) : t('form.providerDefault'),
                hasKey ? t('form.keySet') : t('form.noKey'),
              ].join(' · ')}
              defaultOpen={!model}
              forceOpen={Boolean(
                errors.api_base || errors.api_key || errors.api_version,
              )}
            >
              <div className="space-y-4">
                <ConnectionFields
                  form={form}
                  provider={provider}
                  model={model}
                />
              </div>
            </PanelSection>
            <PanelSection
              title={t('pricingMode.title')}
              summary={t(`pricingMode.${form.watch('pricing_mode')}`)}
              forceOpen={Boolean(errors.rates)}
            >
              <PricingSection form={form} entries={entries} />
            </PanelSection>
            <PanelSection
              title={t('advanced.title')}
              summary={
                weight
                  ? t('advanced.weightSummary', { weight })
                  : t('advanced.weightPlaceholder')
              }
              forceOpen={Boolean(errors.weight)}
            >
              <WeightField form={form} />
            </PanelSection>
          </div>
        </PanelBody>
        <PanelFooter>
          <div className="mr-auto">
            <TestConnect form={form} model={model} />
          </div>
          <Button type="button" variant="ghost" onClick={onClose}>
            {tCommon('cancel')}
          </Button>
          <Button type="submit" disabled={form.formState.isSubmitting}>
            {model ? t('dialog.save') : t('dialog.add')}
            <ModEnterHint />
          </Button>
        </PanelFooter>
      </form>
    </Form>
  );
}

/** `https://api.openai.com/v1` → `api.openai.com`; the text as is otherwise. */
function hostOf(url: string) {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

/** Saves each row in turn; stops at the first the server refuses. */
async function saveAll(payloads: ModelPayload[], model?: Model) {
  for (const payload of payloads) {
    const result = model
      ? await updateModel(model.id, payload)
      : await createModel(payload);
    if (!result.success) return { name: payload.name, error: result.error };
  }
  return null;
}

/** Deployment weight: how much of its name's traffic it takes. */
function WeightField({ form }: { form: UseFormReturn<ModelFormValues> }) {
  const t = useTranslations('models');
  return (
    <FormField
      control={form.control}
      name="weight"
      render={({ field }) => (
        <FormItem>
          <FormLabel>{t('advanced.weight')}</FormLabel>
          <FormControl>
            <Input
              inputMode="numeric"
              placeholder={t('advanced.weightPlaceholder')}
              {...field}
            />
          </FormControl>
          <FormDescription>{t('advanced.weightHint')}</FormDescription>
          <FormMessage />
        </FormItem>
      )}
    />
  );
}

/** "Test connection" with the settings as typed; the result is a toast. */
function TestConnect({
  form,
  model,
}: {
  form: UseFormReturn<ModelFormValues>;
  model?: Model;
}) {
  const t = useTranslations('models');
  const testConnection = useConnectionTest();
  const [testing, setTesting] = useState(false);
  const first = form.watch('mappings')[0];

  return (
    <Button
      type="button"
      variant="outline"
      disabled={!first || testing}
      onClick={async () => {
        setTesting(true);
        const values = form.getValues();
        await testConnection(first.public_name || first.upstream_model, {
          model_id: model?.id,
          provider: values.provider,
          upstream_model: first.upstream_model,
          api_base: values.api_base.trim() || null,
          api_version: values.api_version.trim() || null,
          api_key: values.api_key.trim() || null,
        });
        setTesting(false);
      }}
    >
      {testing ? t('test.running') : t('test.button')}
    </Button>
  );
}
