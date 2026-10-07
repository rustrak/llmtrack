import type { FieldValues, UseFormReturn } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { ApiError } from '@/shared/api/http';
import { applyServerErrors } from '@/shared/lib/form-errors';

/**
 * `applyServerErrors` with the reader's language already bound. `toast`
 * shows the form-level message as a toast, for side panels, where a line at
 * the end of a scrolling body goes unseen.
 */
export function useServerErrors<T extends FieldValues>(
  form: UseFormReturn<T>,
  options: { toast?: boolean } = {},
) {
  const t = useTranslations('formErrors');
  return (error: ApiError) =>
    applyServerErrors(
      form,
      error,
      t,
      options.toast ? (message) => toast.error(message) : undefined,
    );
}
