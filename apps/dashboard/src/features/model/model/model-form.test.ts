import { describe, expect, it } from 'vitest';
import {
  defaultsFor,
  emptyRates,
  type ModelFormValues,
  mappingFor,
  modelFormSchema,
  ratesFrom,
  toModelPayloads,
} from './model-form';

const t = (key: string) => key;

const base: ModelFormValues = {
  provider: 'anthropic',
  mappings: [
    {
      public_name: 'claude-sonnet-4-5',
      upstream_model: 'claude-sonnet-4-5',
      catalog_key: 'claude-sonnet-4-5',
    },
    {
      public_name: 'haiku',
      upstream_model: 'claude-haiku-4-5',
      catalog_key: 'claude-haiku-4-5',
    },
  ],
  api_base: '',
  api_version: '',
  api_key: 'sk-ant',
  clear_api_key: false,
  pricing_mode: 'catalog',
  rates: emptyRates(),
  weight: '',
};

describe('mappingFor', () => {
  it('names a picked model the way clients will ask for it', () => {
    expect(
      mappingFor({ key: 'gemini/gemini-2.5-pro', model: 'gemini-2.5-pro' }),
    ).toEqual({
      public_name: 'gemini-2.5-pro',
      upstream_model: 'gemini-2.5-pro',
      catalog_key: 'gemini/gemini-2.5-pro',
    });
    expect(
      mappingFor({ key: null, model: 'my-finetune' }).catalog_key,
    ).toBeNull();
  });
});

describe('toModelPayloads', () => {
  it('makes one model per mapping, priced by the list', () => {
    const payloads = toModelPayloads(base, 'create');
    expect(payloads).toHaveLength(2);
    expect(payloads[1]).toEqual({
      name: 'haiku',
      provider: 'anthropic',
      upstream_model: 'claude-haiku-4-5',
      api_base: null,
      api_version: null,
      api_key: 'sk-ant',
      catalog_key: 'claude-haiku-4-5',
      pricing: null,
      pricing_mode: 'catalog',
      weight: null,
    });
  });

  it('sends a model with no price on purpose as free, without rates', () => {
    const payloads = toModelPayloads(
      {
        ...base,
        pricing_mode: 'free',
        rates: { ...emptyRates(), input: '3', output: '15' },
      },
      'create',
    );
    expect(payloads[0]).toMatchObject({ pricing_mode: 'free', pricing: null });
  });

  it('gives every deployment the weight typed in', () => {
    const payloads = toModelPayloads({ ...base, weight: ' 3 ' }, 'create');
    expect(payloads.map((p) => p.weight)).toEqual([3, 3]);
  });

  it('applies own rates to every model when chosen', () => {
    const payloads = toModelPayloads(
      {
        ...base,
        pricing_mode: 'custom',
        rates: { ...emptyRates(), input: '3', output: '15', cache_read: '0.3' },
      },
      'create',
    );
    expect(payloads.every((p) => p.pricing?.cache_read === 0.3)).toBe(true);
  });

  it('keeps the stored key on edit unless a new one is typed or it is cleared', () => {
    const edit = { ...base, mappings: base.mappings.slice(0, 1), api_key: '' };
    expect(toModelPayloads(edit, 'edit')[0]).not.toHaveProperty('api_key');
    expect(
      toModelPayloads({ ...edit, clear_api_key: true }, 'edit')[0].api_key,
    ).toBeNull();
  });
});

describe('ratesFrom', () => {
  it('fills the form from a price and back', () => {
    const rates = ratesFrom({
      input: 3,
      output: 15,
      above: { threshold_tokens: 200000, output: 22.5 },
    });
    expect(rates.input).toBe('3');
    expect(rates.above_output).toBe('22.5');
    expect(rates.reasoning).toBe('');
  });
});

describe('modelFormSchema', () => {
  it('needs at least one model and valid public names', () => {
    const schema = modelFormSchema(t);
    expect(schema.safeParse({ ...base, mappings: [] }).success).toBe(false);
    expect(
      schema.safeParse({
        ...base,
        mappings: [{ ...base.mappings[0], public_name: 'has space' }],
      }).success,
    ).toBe(false);
    expect(schema.safeParse(base).success).toBe(true);
  });

  it('checks rates only for own rates', () => {
    const schema = modelFormSchema(t);
    const bad = { ...base, rates: { ...emptyRates(), input: '-1' } };
    expect(schema.safeParse(bad).success).toBe(true);
    expect(schema.safeParse({ ...bad, pricing_mode: 'custom' }).success).toBe(
      false,
    );
  });
});

describe('defaultsFor', () => {
  it('starts empty, or from the model being edited', () => {
    expect(defaultsFor().mappings).toEqual([]);
    const edited = defaultsFor({
      id: 1,
      name: 'm',
      provider: 'openai',
      upstream_model: 'u',
      api_base: null,
      api_version: null,
      effective_api_base: null,
      has_api_key: true,
      catalog_key: null,
      custom_pricing: { input: 1, output: 2 },
      pricing: { input: 1, output: 2 },
      pricing_source: 'custom',
      pricing_mode: 'custom',
      is_active: true,
      created_at: '2026-01-01T00:00:00Z',
      weight: null,
    });
    expect(edited.mappings).toEqual([
      { public_name: 'm', upstream_model: 'u', catalog_key: null },
    ]);
    expect(edited.pricing_mode).toBe('custom');
    expect(edited.rates.output).toBe('2');
  });
});

describe('weight', () => {
  const schema = modelFormSchema(t);
  it('is empty or a whole number of zero or more', () => {
    expect(schema.safeParse({ ...base, weight: '' }).success).toBe(true);
    expect(schema.safeParse({ ...base, weight: '0' }).success).toBe(true);
    expect(schema.safeParse({ ...base, weight: '-1' }).success).toBe(false);
    expect(schema.safeParse({ ...base, weight: '1.5' }).success).toBe(false);
  });

  it('starts from the model being edited', () => {
    expect(defaultsFor().weight).toBe('');
  });
});
