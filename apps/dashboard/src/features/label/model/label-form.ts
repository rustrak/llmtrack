import { z } from 'zod';
import { LABEL_COLORS, type Label } from '@/shared/api/schemas';
import type { Translate } from '@/shared/lib/limits';

/** What the server takes: `services/labels.rs`. */
const MAX_NAME = 50;
const MAX_DESCRIPTION = 200;

export function labelSchema(t: Translate) {
  return z.object({
    name: z
      .string()
      .trim()
      .min(1, t('form.nameRequired'))
      .max(MAX_NAME, t('form.nameTooLong')),
    color: z.enum(LABEL_COLORS),
    description: z
      .string()
      .trim()
      .max(MAX_DESCRIPTION, t('form.descriptionTooLong')),
  });
}
export type LabelValues = z.infer<ReturnType<typeof labelSchema>>;

export interface LabelPayload {
  name: string;
  color: string;
  description: string | null;
}

/** A new label starts grey; an existing one as it is. */
export function labelDefaults(label?: Label): LabelValues {
  return {
    name: label?.name ?? '',
    color: label?.color ?? 'gray',
    description: label?.description ?? '',
  };
}

/** An empty description clears it. */
export function toLabelPayload(values: LabelValues): LabelPayload {
  return {
    name: values.name.trim(),
    color: values.color,
    description: values.description.trim() || null,
  };
}
