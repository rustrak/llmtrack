import type { FieldValues, Path, UseFormReturn } from 'react-hook-form';
import type { ApiError, FieldError } from '@/shared/api/http';

/** Where `FormRootError` looks for the form-level message. */
export const SERVER_ERROR_PATH = 'root.serverError';

/** A translator bound to the `formErrors` namespace. */
export type FormErrorTranslator = (
  key: 'required' | 'invalid' | 'alreadyExists' | 'tooLong',
) => string;

const KEYS: Record<FieldError['code'], Parameters<FormErrorTranslator>[0]> = {
  required: 'required',
  invalid: 'invalid',
  already_exists: 'alreadyExists',
  too_long: 'tooLong',
};

/** The server's message without its `Conflict: ` style prefix. */
export function readableMessage(error: ApiError): string {
  return error.message.replace(/^[A-Z][A-Za-z ]+: /, '');
}

/**
 * Splits a server error into messages for fields the form shows and one
 * message for everything else. A field the server names by JSON path
 * (`pricing.input`) lands on the form field of the same path.
 */
export function serverFieldErrors(
  error: ApiError,
  formFields: readonly string[],
  t: FormErrorTranslator,
): { fields: Record<string, string>; root: string | null } {
  const fields: Record<string, string> = {};
  let unattributed = error.fields.length === 0;
  for (const { field, code } of error.fields) {
    if (formFields.includes(field) && !(field in fields)) {
      fields[field] = t(KEYS[code]);
    } else {
      unattributed = true;
    }
  }
  return { fields, root: unattributed ? readableMessage(error) : null };
}

/** Every dotted path a form's values hold, so nested fields can be pinned. */
function paths(values: unknown, prefix = ''): string[] {
  if (values === null || typeof values !== 'object' || Array.isArray(values)) {
    return prefix ? [prefix] : [];
  }
  return Object.entries(values).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return [path, ...paths(value, path)];
  });
}

/**
 * Pins field errors to their inputs. The form-level message goes to
 * `onRoot` when given (a toast), to `FormRootError` otherwise.
 */
export function applyServerErrors<T extends FieldValues>(
  form: UseFormReturn<T>,
  error: ApiError,
  t: FormErrorTranslator,
  onRoot?: (message: string) => void,
) {
  const { fields, root } = serverFieldErrors(error, paths(form.getValues()), t);
  for (const [name, message] of Object.entries(fields)) {
    form.setError(name as Path<T>, { type: 'server', message });
  }
  if (root && onRoot) {
    onRoot(root);
  } else if (root) {
    form.setError(SERVER_ERROR_PATH as Path<T>, {
      type: 'server',
      message: root,
    });
  }
}
