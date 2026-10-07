import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './shadcn/select';

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
  className,
  'aria-label': ariaLabel,
}: {
  id?: string;
  value: string;
  onChange: (value: string) => void;
  options: readonly Option[];
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  'aria-label'?: string;
}) {
  return (
    <Select
      value={value}
      onValueChange={(next) => onChange(next ?? '')}
      disabled={disabled}
    >
      <SelectTrigger
        id={id}
        className={className ?? 'w-full'}
        aria-label={ariaLabel}
      >
        <SelectValue placeholder={placeholder}>
          {(current) =>
            options.find((o) => o.value === current)?.label ?? placeholder ?? ''
          }
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
