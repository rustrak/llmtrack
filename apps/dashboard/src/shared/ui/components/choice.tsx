import { Select, SelectItem } from '@rustrak/ui';
import { useMemo } from 'react';

export interface Option {
  value: string;
  label: string;
}

/** A select over a plain list of options. */
export function Choice({
  id,
  value,
  onChange,
  options,
  placeholder,
  disabled,
  size,
  className,
  'aria-label': ariaLabel,
  'aria-describedby': ariaDescribedBy,
}: {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  options: readonly Option[];
  placeholder?: string;
  disabled?: boolean;
  size?: 'sm' | 'md';
  className?: string;
  'aria-label'?: string;
  'aria-describedby'?: string;
}) {
  // By content, not identity: callers build `options` inline, and Base UI
  // syncs a new `items` object in an effect that renders again, forever.
  const signature = JSON.stringify(options);
  const items = useMemo(
    () => Object.fromEntries(options.map((o) => [o.value, o.label])),
    [signature],
  );
  return (
    <Select
      id={id}
      value={value || null}
      onValueChange={onChange}
      items={items}
      placeholder={placeholder}
      disabled={disabled}
      size={size}
      className={className}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedBy}
    >
      {options.map((option) => (
        <SelectItem key={option.value} value={option.value}>
          {option.label}
        </SelectItem>
      ))}
    </Select>
  );
}
