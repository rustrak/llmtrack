import { z } from 'zod';
import type { Model, Pricing } from '@/shared/api/schemas';

type Translate = (key: string) => string;

/** Every rate as the form holds it: text, empty meaning "not set". */
export const RATE_FIELDS = [
  'input',
  'output',
  'cache_read',
  'cache_write',
  'cache_write_1h',
  'reasoning',
  'above_threshold',
  'above_input',
  'above_output',
  'above_cache_read',
  'above_cache_write',
  'per_image',
  'per_second',
  'per_million_characters',
] as const;
export type RateField = (typeof RATE_FIELDS)[number];
export type Rates = Record<RateField, string>;

export function emptyRates(): Rates {
  return Object.fromEntries(RATE_FIELDS.map((f) => [f, ''])) as Rates;
}

/** How a model is priced: the price list, its own rates, or not at all. */
export const PRICING_MODES = ['catalog', 'custom', 'free'] as const;
export type PricingMode = (typeof PRICING_MODES)[number];

const isAmount = (v: string) =>
  v.trim() === '' || (Number.isFinite(Number(v)) && Number(v) >= 0);

/**
 * The add-model form: a provider, one or more of its models (each with
 * the public name clients will use), the credentials they share, and custom
 * pricing only when switched on. Prices otherwise come from the price list.
 */
export function modelFormSchema(t: Translate) {
  const mapping = z.object({
    public_name: z
      .string()
      .trim()
      .min(1, t('form.nameRequired'))
      .max(100)
      .regex(/^[A-Za-z0-9._:/-]+$/, t('form.nameInvalid')),
    upstream_model: z
      .string()
      .trim()
      .min(1, t('form.upstreamRequired'))
      .max(200),
    catalog_key: z.string().nullable(),
  });
  return z
    .object({
      provider: z.string().min(1, t('form.providerRequired')),
      mappings: z.array(mapping).min(1, t('form.modelsRequired')),
      api_base: z.string().trim(),
      api_version: z.string().trim(),
      api_key: z.string(),
      clear_api_key: z.boolean(),
      /**
       * The price list, own rates, or no price at all (free on purpose).
       */
      pricing_mode: z.enum(PRICING_MODES),
      rates: z.object(
        Object.fromEntries(RATE_FIELDS.map((f) => [f, z.string()])) as Record<
          RateField,
          z.ZodString
        >,
      ),
      /** Deployment weight: share of its name's traffic. */
      weight: z
        .string()
        .trim()
        .refine((v) => v === '' || /^\d+$/.test(v), {
          message: t('form.weightInvalid'),
        }),
    })
    .superRefine((values, ctx) => {
      if (values.pricing_mode !== 'custom') return;
      for (const field of RATE_FIELDS) {
        if (!isAmount(values.rates[field])) {
          ctx.addIssue({
            code: 'custom',
            path: ['rates', field],
            message: t('form.rateInvalid'),
          });
        }
      }
    });
}
export type ModelFormValues = z.infer<ReturnType<typeof modelFormSchema>>;
export type Mapping = ModelFormValues['mappings'][number];

/** A row for a model picked from the list (or typed, with no key). */
export function mappingFor(entry: {
  key: string | null;
  model: string;
}): Mapping {
  return {
    public_name: entry.model,
    upstream_model: entry.model,
    catalog_key: entry.key,
  };
}

export interface ModelPayload {
  name: string;
  provider: string;
  upstream_model: string;
  api_base: string | null;
  api_version: string | null;
  catalog_key: string | null;
  pricing: Pricing | null;
  pricing_mode: PricingMode;
  weight: number | null;
  api_key?: string | null;
}

const number = (v: string) => (v.trim() === '' ? undefined : Number(v));

/** Own rates from the form; blank optional rates are left out. */
export function toPricing(rates: Rates): Pricing {
  const pricing: Pricing = {
    input: number(rates.input) ?? 0,
    output: number(rates.output) ?? 0,
  };
  for (const field of [
    'cache_read',
    'cache_write',
    'cache_write_1h',
    'reasoning',
    'per_image',
    'per_second',
    'per_million_characters',
  ] as const) {
    const value = number(rates[field]);
    if (value !== undefined) pricing[field] = value;
  }
  const threshold = number(rates.above_threshold);
  if (threshold !== undefined && threshold > 0) {
    const above: NonNullable<Pricing['above']> = {
      threshold_tokens: threshold,
    };
    for (const [field, key] of [
      ['above_input', 'input'],
      ['above_output', 'output'],
      ['above_cache_read', 'cache_read'],
      ['above_cache_write', 'cache_write'],
    ] as const) {
      const value = number(rates[field]);
      if (value !== undefined) above[key] = value;
    }
    pricing.above = above;
  }
  return pricing;
}

/** The form's rates from a price, to start editing from the list's. */
export function ratesFrom(pricing: Pricing): Rates {
  const text = (v: number | undefined) => (v === undefined ? '' : String(v));
  return {
    input: text(pricing.input),
    output: text(pricing.output),
    cache_read: text(pricing.cache_read),
    cache_write: text(pricing.cache_write),
    cache_write_1h: text(pricing.cache_write_1h),
    reasoning: text(pricing.reasoning),
    above_threshold: text(pricing.above?.threshold_tokens),
    above_input: text(pricing.above?.input),
    above_output: text(pricing.above?.output),
    above_cache_read: text(pricing.above?.cache_read),
    above_cache_write: text(pricing.above?.cache_write),
    per_image: text(pricing.per_image),
    per_second: text(pricing.per_second),
    per_million_characters: text(pricing.per_million_characters),
  };
}

/**
 * One payload per row. On edit an empty key means "keep the stored one":
 * the server never sends it back, so the form cannot show it.
 */
export function toModelPayloads(
  values: ModelFormValues,
  mode: 'create' | 'edit',
): ModelPayload[] {
  const key = values.api_key.trim();
  return values.mappings.map((row) => {
    const payload: ModelPayload = {
      name: row.public_name.trim(),
      provider: values.provider,
      upstream_model: row.upstream_model.trim(),
      api_base: values.api_base.trim() || null,
      api_version: values.api_version.trim() || null,
      catalog_key: row.catalog_key,
      pricing:
        values.pricing_mode === 'custom' ? toPricing(values.rates) : null,
      pricing_mode: values.pricing_mode,
      weight: values.weight.trim() === '' ? null : Number(values.weight),
    };
    if (mode === 'edit' && values.clear_api_key) {
      payload.api_key = null;
    } else if (key !== '') {
      payload.api_key = key;
    }
    return payload;
  });
}

/** The form's starting values: empty, or the model being edited. */
export function defaultsFor(model?: Model): ModelFormValues {
  return {
    provider: model?.provider ?? 'openai',
    mappings: model
      ? [
          {
            public_name: model.name,
            upstream_model: model.upstream_model,
            catalog_key: model.catalog_key,
          },
        ]
      : [],
    api_base: model?.api_base ?? '',
    api_version: model?.api_version ?? '',
    api_key: '',
    clear_api_key: false,
    pricing_mode: model?.pricing_mode ?? 'catalog',
    rates: model?.custom_pricing
      ? ratesFrom(model.custom_pricing)
      : emptyRates(),
    weight: model?.weight == null ? '' : String(model.weight),
  };
}
