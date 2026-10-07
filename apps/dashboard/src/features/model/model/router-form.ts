import { z } from 'zod';
import type { FallbackList, RouterSettings } from '@/shared/api/schemas';

type Translate = (key: string) => string;

/** One line of a fallback editor: a model (or `*`) and where it goes. */
export interface FallbackRow {
  model: string;
  fallbacks: string[];
}

export const FALLBACK_KINDS = [
  'fallbacks',
  'context_window_fallbacks',
  'content_policy_fallbacks',
] as const;
export type FallbackKind = (typeof FALLBACK_KINDS)[number];

export function toFallbackRows(list: FallbackList): FallbackRow[] {
  return list.flatMap((entry) =>
    Object.entries(entry).map(([model, fallbacks]) => ({ model, fallbacks })),
  );
}

export function fromFallbackRows(rows: FallbackRow[]): FallbackList {
  return rows
    .filter((row) => row.model !== '' && row.fallbacks.length > 0)
    .map((row) => ({ [row.model]: row.fallbacks }));
}

const whole = (t: Translate, max: number) =>
  z
    .string()
    .trim()
    .refine((v) => /^\d+$/.test(v) && Number(v) <= max, {
      message: t('router.numberInvalid'),
    });

export function routerFormSchema(t: Translate) {
  const rows = z.array(
    z.object({ model: z.string(), fallbacks: z.array(z.string()) }),
  );
  return z.object({
    num_retries: whole(t, 10),
    allowed_fails: whole(t, 1000),
    cooldown_time: whole(t, 3600),
    fallbacks: rows,
    context_window_fallbacks: rows,
    content_policy_fallbacks: rows,
  });
}
export type RouterFormValues = z.infer<ReturnType<typeof routerFormSchema>>;

export function routerDefaults(settings: RouterSettings): RouterFormValues {
  return {
    num_retries: String(settings.num_retries),
    allowed_fails: String(settings.allowed_fails),
    cooldown_time: String(settings.cooldown_time),
    fallbacks: toFallbackRows(settings.fallbacks),
    context_window_fallbacks: toFallbackRows(settings.context_window_fallbacks),
    content_policy_fallbacks: toFallbackRows(settings.content_policy_fallbacks),
  };
}

export function toRouterPayload(values: RouterFormValues): RouterSettings {
  return {
    num_retries: Number(values.num_retries),
    allowed_fails: Number(values.allowed_fails),
    cooldown_time: Number(values.cooldown_time),
    fallbacks: fromFallbackRows(values.fallbacks),
    context_window_fallbacks: fromFallbackRows(values.context_window_fallbacks),
    content_policy_fallbacks: fromFallbackRows(values.content_policy_fallbacks),
  };
}
