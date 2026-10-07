import { Input } from '@/shared/ui/components/shadcn/input';

/** A free-text filter that applies on Enter or when it loses focus. */
export function TextFilter({
  value,
  label,
  onChange,
}: {
  value: string | undefined;
  label: string;
  onChange: (value: string | undefined) => void;
}) {
  const commit = (text: string) => {
    const next = text.trim() || undefined;
    if (next !== value) onChange(next);
  };
  return (
    <Input
      key={value ?? ''}
      aria-label={label}
      placeholder={label}
      className="h-8 w-36"
      defaultValue={value ?? ''}
      onBlur={(e) => commit(e.target.value)}
      onKeyDown={(e) => e.key === 'Enter' && commit(e.currentTarget.value)}
    />
  );
}
