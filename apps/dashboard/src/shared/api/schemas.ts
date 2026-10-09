import { z } from 'zod';

/** Everything the server answers with, in one place: the API's contract. */

export const userSchema = z.object({
  id: z.number(),
  email: z.string(),
  role: z.enum(['admin', 'member']),
  is_active: z.boolean(),
  created_at: z.string(),
  last_login: z.string().nullable(),
  name: z.string().nullable().optional(),
  language: z.string().nullable().optional(),
  timezone: z.string().nullable().optional(),
  currency: z.string().nullable().optional(),
  currency_rate: z.number().nullable().optional(),
});
export type User = z.infer<typeof userSchema>;

/** A row of the users table: what their keys spent, how many they have. */
export const userListItemSchema = userSchema.extend({
  spend_usd: z.number(),
  key_count: z.number(),
});
export type UserListItem = z.infer<typeof userListItemSchema>;

export const invitationSchema = z.object({
  token: z.string(),
  email: z.string(),
  role: z.enum(['admin', 'member']),
  status: z.enum(['pending', 'accepted', 'revoked']),
  expires_at: z.string(),
  created_at: z.string(),
});
export type Invitation = z.infer<typeof invitationSchema>;

/** What the public invite page learns from a token. */
export const invitationInfoSchema = z.object({
  email: z.string(),
  role: z.enum(['admin', 'member']),
  expires_at: z.string(),
});
export type InvitationInfo = z.infer<typeof invitationInfoSchema>;

export const modelRefSchema = z.object({ id: z.number(), name: z.string() });
export type ModelRef = z.infer<typeof modelRefSchema>;

export const providerSchema = z.object({
  id: z.string(),
  label: z.string(),
  wire: z.enum(['open_ai', 'anthropic', 'azure']),
  default_api_base: z.string().nullable(),
});
export type Provider = z.infer<typeof providerSchema>;

const aboveTierSchema = z.object({
  threshold_tokens: z.number(),
  input: z.number().optional(),
  output: z.number().optional(),
  cache_read: z.number().optional(),
  cache_write: z.number().optional(),
});

/** Rates in dollars: per 1M tokens, per image or second, per 1M characters. */
export const pricingSchema = z.object({
  input: z.number(),
  output: z.number(),
  cache_read: z.number().optional(),
  cache_write: z.number().optional(),
  cache_write_1h: z.number().optional(),
  reasoning: z.number().optional(),
  above: aboveTierSchema.optional(),
  per_image: z.number().optional(),
  per_second: z.number().optional(),
  per_million_characters: z.number().optional(),
});
export type Pricing = z.infer<typeof pricingSchema>;

export const modelSchema = z.object({
  id: z.number(),
  name: z.string(),
  provider: z.string(),
  upstream_model: z.string(),
  api_base: z.string().nullable(),
  api_version: z.string().nullable(),
  effective_api_base: z.string().nullable(),
  has_api_key: z.boolean(),
  catalog_key: z.string().nullable(),
  custom_pricing: pricingSchema.nullable(),
  pricing: pricingSchema.nullable(),
  /** Where the price comes from; `none` is a list model the list lacks. */
  pricing_source: z.enum(['custom', 'catalog', 'free', 'none']),
  pricing_mode: z.enum(['catalog', 'custom', 'free']),
  is_active: z.boolean(),
  created_at: z.string(),
  /** Share of its name's traffic among the name's deployments. */
  weight: z.number().nullable(),
});
export type Model = z.infer<typeof modelSchema>;

/** Fallbacks: `[{"gpt-4o": ["claude"]}, {"*": ["mini"]}]`. */
export const fallbackListSchema = z.array(
  z.record(z.string(), z.array(z.string())),
);
export type FallbackList = z.infer<typeof fallbackListSchema>;

export const routerSettingsSchema = z.object({
  num_retries: z.number(),
  allowed_fails: z.number(),
  cooldown_time: z.number(),
  fallbacks: fallbackListSchema,
  context_window_fallbacks: fallbackListSchema,
  content_policy_fallbacks: fallbackListSchema,
});
export type RouterSettings = z.infer<typeof routerSettingsSchema>;

export const catalogEntrySchema = z.object({
  key: z.string(),
  /** The id the provider expects (the key without the list's prefix). */
  model: z.string(),
  provider: z.string(),
  mode: z.string(),
  pricing: pricingSchema,
  max_input_tokens: z.number().nullable(),
  max_output_tokens: z.number().nullable(),
  supports_reasoning: z.boolean(),
  supports_prompt_caching: z.boolean(),
  supports_vision: z.boolean(),
  supports_function_calling: z.boolean(),
});
export type CatalogEntry = z.infer<typeof catalogEntrySchema>;

export const catalogStatusSchema = z.object({
  source: z.enum(['builtin', 'synced']),
  entries: z.number(),
  synced_at: z.string().nullable(),
  url: z.string(),
});
export type CatalogStatus = z.infer<typeof catalogStatusSchema>;

export const settingsSchema = z.object({
  public_url: z.string().nullable(),
  price_catalog_url: z.string().optional(),
});
export type Settings = z.infer<typeof settingsSchema>;

export const teamSchema = z.object({
  id: z.number(),
  name: z.string(),
  max_budget_usd: z.number().nullable(),
  spend_usd: z.number(),
  budget_duration: z.string().nullable(),
  budget_reset_at: z.string().nullable(),
  rpm_limit: z.number().nullable(),
  tpm_limit: z.number().nullable(),
  max_parallel_requests: z.number().nullable(),
  created_at: z.string(),
  member_count: z.number(),
  key_count: z.number(),
  /** Every model on the proxy; otherwise only `models` (empty: none). */
  all_models: z.boolean(),
  models: z.array(modelRefSchema),
  my_role: z.enum(['admin', 'member']).nullable(),
});
export type Team = z.infer<typeof teamSchema>;

export const memberSchema = z.object({
  user_id: z.number(),
  email: z.string(),
  role: z.enum(['admin', 'member']),
});
export type Member = z.infer<typeof memberSchema>;

/** Someone who spends through a team's keys without a dashboard account. */
export const personSchema = z.object({
  id: z.number(),
  /** Null until they are placed in a team. */
  team_id: z.number().nullable(),
  team_name: z.string().nullable(),
  name: z.string(),
  email: z.string().nullable(),
  created_at: z.string(),
  key_count: z.number(),
  spend_usd: z.number(),
});
export type Person = z.infer<typeof personSchema>;

export const teamDetailSchema = teamSchema.extend({
  members: z.array(memberSchema),
  people: z.array(personSchema),
});
export type TeamDetail = z.infer<typeof teamDetailSchema>;

/** The palette a label is drawn in: `models/label.rs` on the server. */
export const LABEL_COLORS = [
  'gray',
  'red',
  'orange',
  'amber',
  'green',
  'teal',
  'blue',
  'violet',
  'pink',
] as const;
export type LabelColor = (typeof LABEL_COLORS)[number];

/** A label as a key shows it. */
export const labelRefSchema = z.object({
  id: z.number(),
  name: z.string(),
  color: z.enum(LABEL_COLORS),
});
export type LabelRef = z.infer<typeof labelRefSchema>;

export const labelSchema = labelRefSchema.extend({
  description: z.string().nullable(),
  created_at: z.string(),
  /** Active keys that carry it. */
  key_count: z.number(),
});
export type Label = z.infer<typeof labelSchema>;

export const keySchema = z.object({
  id: z.number(),
  name: z.string(),
  /** Null for a personal key, which belongs to `user_id`. */
  team_id: z.number().nullable(),
  team_name: z.string().nullable(),
  user_id: z.number().nullable(),
  owner_email: z.string().nullable(),
  /** The team's person the key is for. */
  person_id: z.number().nullable(),
  person_name: z.string().nullable(),
  key_hint: z.string(),
  models: z.array(modelRefSchema),
  labels: z.array(labelRefSchema),
  max_budget_usd: z.number().nullable(),
  spend_usd: z.number(),
  budget_duration: z.string().nullable(),
  budget_reset_at: z.string().nullable(),
  rpm_limit: z.number().nullable(),
  tpm_limit: z.number().nullable(),
  max_parallel_requests: z.number().nullable(),
  blocked: z.boolean(),
  /** Keeps each request's body and reply. */
  log_bodies: z.boolean(),
  /** Days they are kept; null keeps them until deleted. */
  body_retention_days: z.number().nullable(),
  expires_at: z.string().nullable(),
  last_used_at: z.string().nullable(),
  created_at: z.string(),
  created_by: z.number().nullable(),
  created_by_email: z.string().nullable(),
});
export type ApiKey = z.infer<typeof keySchema>;

/** One page of a list: `models/list.rs` on the server. */
export const pagedSchema = <T extends z.ZodTypeAny>(item: T) =>
  z.object({
    data: z.array(item),
    total: z.number(),
    page: z.number(),
    per_page: z.number(),
  });
export type Paged<T> = {
  data: T[];
  total: number;
  page: number;
  per_page: number;
};

export const createdKeySchema = keySchema.extend({ key: z.string() });
export type CreatedKey = z.infer<typeof createdKeySchema>;

const totalsShape = {
  cost_usd: z.number(),
  requests: z.number(),
  failed_requests: z.number(),
  prompt_tokens: z.number(),
  completion_tokens: z.number(),
  cached_tokens: z.number(),
  cache_write_tokens: z.number(),
  reasoning_tokens: z.number(),
  /** Summed over the requests: divide by `requests` for the average. */
  latency_ms: z.number(),
};
export const totalsSchema = z.object(totalsShape);
export type Totals = z.infer<typeof totalsSchema>;

export const usageSchema = z.object({
  from: z.string(),
  to: z.string(),
  totals: totalsSchema,
  /** The same filters over the equal window just before `from`. */
  previous: totalsSchema,
  /** Per day and group of the report's `group_by`; quiet days absent. */
  series: z.array(
    z.object({
      day: z.string(),
      key: z.string(),
      label: z.string().nullable(),
      ...totalsShape,
    }),
  ),
  daily: z.array(z.object({ day: z.string(), ...totalsShape })),
  by_team: z.array(
    z.object({
      /** Null: personal keys, which bill to no team. */
      team_id: z.number().nullable(),
      team_name: z.string().nullable(),
      ...totalsShape,
    }),
  ),
  by_key: z.array(
    z.object({
      key_id: z.number(),
      key_name: z.string(),
      key_hint: z.string(),
      team_name: z.string().nullable(),
      ...totalsShape,
    }),
  ),
  /** A team's people; keys of no one are left out. */
  by_person: z.array(
    z.object({
      person_id: z.number(),
      person_name: z.string(),
      team_name: z.string().nullable(),
      ...totalsShape,
    }),
  ),
  by_model: z.array(z.object({ model_name: z.string(), ...totalsShape })),
  /** Customers: the `user` a request named. */
  by_end_user: z.array(z.object({ end_user: z.string(), ...totalsShape })),
  /** A request counts once under each of its tags. */
  by_tag: z.array(z.object({ tag: z.string(), ...totalsShape })),
});
export type Usage = z.infer<typeof usageSchema>;

export const logLineSchema = z.object({
  id: z.number(),
  request_id: z.string(),
  created_at: z.string(),
  team_id: z.number().nullable(),
  team_name: z.string().nullable(),
  user_id: z.number().nullable(),
  key_id: z.number(),
  /** The deployment that answered. */
  model_id: z.number().nullable(),
  end_user: z.string().nullable(),
  tags: z.array(z.string()),
  key_name: z.string().nullable(),
  key_hint: z.string().nullable(),
  model_name: z.string(),
  provider: z.string(),
  status_code: z.number(),
  prompt_tokens: z.number(),
  completion_tokens: z.number(),
  cached_tokens: z.number(),
  cache_write_tokens: z.number(),
  reasoning_tokens: z.number(),
  endpoint: z.string(),
  total_tokens: z.number(),
  cost_usd: z.number(),
  latency_ms: z.number(),
  stream: z.boolean(),
  error: z.string().nullable(),
  /** Its key kept the request and the reply. */
  has_body: z.boolean(),
});
export type LogLine = z.infer<typeof logLineSchema>;

/** What a key that keeps bodies kept: the request as sent, the reply as received. */
export const bodySchema = z.object({
  request: z.unknown(),
  response: z.unknown().nullable(),
});
export type Body = z.infer<typeof bodySchema>;

export const probeSchema = z.object({
  ok: z.boolean(),
  status: z.number().nullable(),
  message: z.string(),
  latency_ms: z.number(),
});
export type Probe = z.infer<typeof probeSchema>;
