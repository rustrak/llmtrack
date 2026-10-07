import type { UseFormReturn } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import type { Model, Provider } from '@/shared/api/schemas';
import { PasswordInput } from '@/shared/ui/components/password-input';
import { Checkbox } from '@/shared/ui/components/shadcn/checkbox';
import {
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
import type { ModelFormValues } from '../../model/model-form';

/** Where the provider is and the key to reach it, shared by every row. */
export function ConnectionFields({
  form,
  provider,
  model,
}: {
  form: UseFormReturn<ModelFormValues>;
  provider?: Provider;
  model?: Model;
}) {
  const t = useTranslations('models');
  const azure = provider?.wire === 'azure';
  const basePlaceholder =
    provider?.default_api_base ??
    (azure
      ? 'https://my-resource.openai.azure.com'
      : 'http://vllm.internal:8000/v1');

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <FormField
          control={form.control}
          name="api_base"
          render={({ field }) => (
            <FormItem className={azure ? '' : 'col-span-2'}>
              <FormLabel>{t('form.apiBase')}</FormLabel>
              <FormControl>
                <Input
                  className="font-mono"
                  placeholder={basePlaceholder}
                  {...field}
                />
              </FormControl>
              <FormDescription>
                {provider?.default_api_base
                  ? t('form.apiBaseDefault')
                  : t('form.apiBaseRequired')}
              </FormDescription>
              <FormMessage />
            </FormItem>
          )}
        />
        {azure && (
          <FormField
            control={form.control}
            name="api_version"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.apiVersion')}</FormLabel>
                <FormControl>
                  <Input
                    className="font-mono"
                    placeholder="2024-10-21"
                    {...field}
                  />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
        )}
      </div>
      <FormField
        control={form.control}
        name="api_key"
        render={({ field }) => (
          <FormItem>
            <FormLabel>{t('form.apiKey')}</FormLabel>
            <FormControl>
              <PasswordInput
                autoComplete="off"
                placeholder={
                  model?.has_api_key ? t('form.apiKeyStored') : 'sk-…'
                }
                {...field}
              />
            </FormControl>
            <FormDescription>{t('form.apiKeyHint')}</FormDescription>
            <FormMessage />
          </FormItem>
        )}
      />
      {model?.has_api_key && (
        <FormField
          control={form.control}
          name="clear_api_key"
          render={({ field }) => (
            <label className="flex items-center gap-2 text-sm">
              <Checkbox
                checked={field.value}
                onCheckedChange={field.onChange}
              />
              {t('form.clearApiKey')}
            </label>
          )}
        />
      )}
    </div>
  );
}
