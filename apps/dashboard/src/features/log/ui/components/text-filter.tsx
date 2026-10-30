import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { type ListFilter, parseList } from '@/shared/lib/filters';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';

/**
 * A free-text list filter (`alice, bob`) that applies on Enter or when it
 * loses focus, with a "not" toggle that leaves those values out instead.
 */
export function TextFilter({
  filter,
  label,
  onChange,
}: {
  filter: ListFilter;
  label: string;
  onChange: (filter: ListFilter) => void;
}) {
  const t = useTranslations('table');
  // The mode outlives an empty box, which the URL cannot hold.
  const [wantsExclude, setWantsExclude] = useState(filter.exclude);
  const exclude = filter.values.length > 0 ? filter.exclude : wantsExclude;
  const text = filter.values.join(', ');
  const commit = (typed: string) => {
    const values = parseList(typed);
    if (values.join(', ') !== text) onChange({ values, exclude });
  };
  return (
    <div className="flex items-center gap-1">
      <Button
        variant={exclude ? 'secondary' : 'ghost'}
        size="sm"
        className="h-8 px-2"
        aria-pressed={exclude}
        aria-label={t('excludeValues', { filter: label })}
        title={t('excludeValues', { filter: label })}
        onClick={() => {
          setWantsExclude(!exclude);
          if (filter.values.length > 0)
            onChange({ values: filter.values, exclude: !exclude });
        }}
      >
        {t('filterNot')}
      </Button>
      <Input
        key={text}
        aria-label={label}
        placeholder={label}
        className="h-8 w-40"
        defaultValue={text}
        onBlur={(e) => commit(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && commit(e.currentTarget.value)}
      />
    </div>
  );
}
