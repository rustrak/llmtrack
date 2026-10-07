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
import type { TeamPayload } from '../api/mutations';

export function teamFormSchema(t: Translate) {
  return z.object({
    name: z.string().trim().min(1, t('form.nameRequired')).max(100),
    max_budget_usd: budgetField(t),
    budget_duration: z.string(),
    rpm_limit: limitField(t),
    tpm_limit: limitField(t),
    max_parallel_requests: limitField(t),
    /** "All models": every model, including later ones. */
    all_models: z.boolean(),
    models: z.array(z.number()),
  });
}
export type TeamFormValues = z.infer<ReturnType<typeof teamFormSchema>>;

export function teamDefaults(
  team?: {
    name: string;
    max_budget_usd: number | null;
    budget_duration: string | null;
    rpm_limit: number | null;
    tpm_limit: number | null;
    max_parallel_requests: number | null;
    all_models: boolean;
    models: { id: number }[];
  },
  perUsd = 1,
): TeamFormValues {
  const text = (n: number | null | undefined) => (n == null ? '' : String(n));
  return {
    name: team?.name ?? '',
    max_budget_usd: budgetText(team?.max_budget_usd, perUsd),
    budget_duration: team?.budget_duration ?? 'none',
    rpm_limit: text(team?.rpm_limit),
    tpm_limit: text(team?.tpm_limit),
    max_parallel_requests: text(team?.max_parallel_requests),
    all_models: team?.all_models ?? true,
    models: team?.models.map((m) => m.id) ?? [],
  };
}

export function toTeamPayload(values: TeamFormValues, perUsd = 1): TeamPayload {
  return {
    name: values.name.trim(),
    max_budget_usd: budgetOrNull(values.max_budget_usd, perUsd),
    budget_duration: periodOrNull(values.budget_duration),
    rpm_limit: numberOrNull(values.rpm_limit),
    tpm_limit: numberOrNull(values.tpm_limit),
    max_parallel_requests: numberOrNull(values.max_parallel_requests),
    all_models: values.all_models,
    models: values.all_models ? [] : values.models,
  };
}
