import { z } from 'zod';
import {
  budgetField,
  budgetOrNull,
  budgetText,
  limitField,
  numberOrNull,
  periodOrNull,
  type Translate,
} from '@/shared/lib/limits';

const DAY_MS = 24 * 60 * 60 * 1000;

/** Days until expiry; labelled in the dialog. */
export { BUDGET_PERIODS } from '@/shared/lib/limits';

export const EXPIRY_OPTIONS = ['never', '7', '30', '90', '365'] as const;

/** A personal key, owned by whoever creates it, instead of a team's. */
export const PERSONAL = 'personal';

function limitsShape(t: Translate) {
  return {
    max_budget_usd: budgetField(t),
    budget_duration: z.string(),
    rpm_limit: limitField(t),
    tpm_limit: limitField(t),
    max_parallel_requests: limitField(t),
  };
}

export function createKeySchema(t: Translate) {
  return z.object({
    team_id: z.string(),
    /** One of the team's people, or empty for no one. */
    person_id: z.string(),
    name: z.string().trim().min(1, t('form.nameRequired')).max(100),
    models: z.array(z.number()),
    ...limitsShape(t),
    expires: z.enum(EXPIRY_OPTIONS),
  });
}
export type CreateKeyValues = z.infer<ReturnType<typeof createKeySchema>>;

export function updateKeySchema(t: Translate) {
  return z.object({
    name: z.string().trim().min(1, t('form.nameRequired')).max(100),
    models: z.array(z.number()),
    person_id: z.string(),
    ...limitsShape(t),
    /** `yyyy-mm-dd`, or empty for never. */
    expires_on: z.string(),
  });
}
export type UpdateKeyValues = z.infer<ReturnType<typeof updateKeySchema>>;

export interface CreateKeyPayload {
  team_id?: number;
  person_id?: number;
  name: string;
  models: number[];
  max_budget_usd?: number;
  budget_duration?: string;
  rpm_limit?: number;
  tpm_limit?: number;
  max_parallel_requests?: number;
  expires_at?: string;
}

export function toCreateKeyPayload(
  values: CreateKeyValues,
  now = new Date(),
  perUsd = 1,
): CreateKeyPayload {
  const optional = {
    team_id: values.team_id === PERSONAL ? null : Number(values.team_id),
    person_id:
      values.team_id === PERSONAL ? null : numberOrNull(values.person_id),
    max_budget_usd: budgetOrNull(values.max_budget_usd, perUsd),
    budget_duration: periodOrNull(values.budget_duration),
    rpm_limit: numberOrNull(values.rpm_limit),
    tpm_limit: numberOrNull(values.tpm_limit),
    max_parallel_requests: numberOrNull(values.max_parallel_requests),
    expires_at:
      values.expires === 'never'
        ? null
        : new Date(
            now.getTime() + Number(values.expires) * DAY_MS,
          ).toISOString(),
  };
  const payload: CreateKeyPayload = {
    name: values.name.trim(),
    models: values.models,
  };
  for (const [field, value] of Object.entries(optional)) {
    if (value !== null) Object.assign(payload, { [field]: value });
  }
  return payload;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** A timestamp as the local `yyyy-mm-dd` a date input shows. */
function localDay(iso: string) {
  const d = new Date(iso);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function editDefaults(
  key: {
    name: string;
    models: { id: number }[];
    max_budget_usd: number | null;
    budget_duration: string | null;
    rpm_limit: number | null;
    tpm_limit: number | null;
    max_parallel_requests: number | null;
    expires_at: string | null;
    person_id: number | null;
  },
  perUsd = 1,
): UpdateKeyValues {
  const text = (n: number | null) => (n === null ? '' : String(n));
  return {
    name: key.name,
    models: key.models.map((m) => m.id),
    person_id: text(key.person_id),
    max_budget_usd: budgetText(key.max_budget_usd, perUsd),
    budget_duration: key.budget_duration ?? 'none',
    rpm_limit: text(key.rpm_limit),
    tpm_limit: text(key.tpm_limit),
    max_parallel_requests: text(key.max_parallel_requests),
    expires_on: key.expires_at ? localDay(key.expires_at) : '',
  };
}

/**
 * The PATCH body: empty fields clear with null. The expiry is sent only
 * when it changed, so saving a key that already expired does not trip on
 * its own past date.
 */
export function toUpdateKeyPayload(
  values: UpdateKeyValues,
  expiresOn: string,
  perUsd = 1,
) {
  const payload: {
    name: string;
    models: number[];
    person_id: number | null;
    max_budget_usd: number | null;
    budget_duration: string | null;
    rpm_limit: number | null;
    tpm_limit: number | null;
    max_parallel_requests: number | null;
    expires_at?: string | null;
  } = {
    name: values.name.trim(),
    models: values.models,
    person_id: numberOrNull(values.person_id),
    max_budget_usd: budgetOrNull(values.max_budget_usd, perUsd),
    budget_duration: periodOrNull(values.budget_duration),
    rpm_limit: numberOrNull(values.rpm_limit),
    tpm_limit: numberOrNull(values.tpm_limit),
    max_parallel_requests: numberOrNull(values.max_parallel_requests),
  };
  if (values.expires_on !== expiresOn) {
    payload.expires_at = values.expires_on
      ? new Date(`${values.expires_on}T23:59:59`).toISOString()
      : null;
  }
  return payload;
}

/** How to call the gateway with a new key, in the usual dialects. */
export function snippets(origin: string, key: string, model: string) {
  const base = `${origin}/v1`;
  return {
    curl: `curl ${base}/chat/completions \\
  -H "Authorization: Bearer ${key}" \\
  -H "Content-Type: application/json" \\
  -d '{"model": "${model}", "messages": [{"role": "user", "content": "Hello"}]}'`,
    python: `from openai import OpenAI

client = OpenAI(base_url="${base}", api_key="${key}")
reply = client.chat.completions.create(
    model="${model}",
    messages=[{"role": "user", "content": "Hello"}],
)
print(reply.choices[0].message.content)`,
    node: `import OpenAI from 'openai';

const client = new OpenAI({ baseURL: '${base}', apiKey: '${key}' });
const reply = await client.chat.completions.create({
  model: '${model}',
  messages: [{ role: 'user', content: 'Hello' }],
});
console.log(reply.choices[0].message.content);`,
    anthropic: `export ANTHROPIC_BASE_URL="${origin}"
export ANTHROPIC_AUTH_TOKEN="${key}"
export ANTHROPIC_MODEL="${model}"
claude`,
  };
}
