import type React from 'react';
import { useTranslations } from 'use-intl';
import type { LogLine } from '@/shared/api/schemas';
import { CopyButton } from '@/shared/ui/components/copy-button';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/shared/ui/components/shadcn/sheet';
import { useMoney } from '@/shared/ui/hooks/use-money';

export function StatusBadge({ code }: { code: number }) {
  const t = useTranslations('logs');
  if (code < 400) return <Badge variant="secondary">{code}</Badge>;
  return (
    <Badge variant="destructive">{code === 499 ? t('clientLeft') : code}</Badge>
  );
}

/** Who a request billed to: its team, or the owner of a personal key. */
export function billedTo(row: LogLine, personal: string) {
  if (row.team_name) return row.team_name;
  return row.team_id === null ? personal : `#${row.team_id}`;
}

/** Everything logged about one request. */
export function LogDetail({
  row,
  onClose,
}: {
  row: LogLine | null;
  onClose: () => void;
}) {
  const t = useTranslations('logs');
  const money = useMoney();
  return (
    <Sheet open={row !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-full overflow-y-auto sm:max-w-md">
        {row && (
          <>
            <SheetHeader>
              <SheetTitle className="flex items-center gap-2">
                <span className="font-mono">{row.model_name}</span>
                <StatusBadge code={row.status_code} />
              </SheetTitle>
              <SheetDescription>
                {money.dateTime(row.created_at)}
              </SheetDescription>
            </SheetHeader>
            <dl className="space-y-4 px-4 pb-6 text-sm">
              <Field label={t('detail.requestId')}>
                <span className="flex items-center gap-2">
                  <code className="truncate font-mono text-xs">
                    {row.request_id}
                  </code>
                  <CopyButton text={row.request_id} />
                </span>
              </Field>
              <Field label={t('detail.endpoint')}>
                /v1/{row.endpoint}
                {row.stream && ` · ${t('stream')}`}
              </Field>
              <Field label={t('detail.provider')}>
                {row.provider}
                {row.model_id !== null && (
                  <span className="ml-2 text-xs text-muted-foreground">
                    {t('detail.deployment', { id: row.model_id })}
                  </span>
                )}
              </Field>
              <Field label={t('table.team')}>
                {billedTo(row, t('personal'))}
              </Field>
              <Field label={t('table.key')}>
                {row.key_name ?? `#${row.key_id}`}{' '}
                <span className="font-mono text-xs text-muted-foreground">
                  {row.key_hint}
                </span>
              </Field>
              {row.end_user && (
                <Field label={t('detail.endUser')}>{row.end_user}</Field>
              )}
              {row.tags.length > 0 && (
                <Field label={t('detail.tags')}>
                  <span className="flex flex-wrap gap-1">
                    {row.tags.map((tag) => (
                      <Badge key={tag} variant="outline">
                        {tag}
                      </Badge>
                    ))}
                  </span>
                </Field>
              )}
              <div className="grid grid-cols-2 gap-4">
                <Field label={t('detail.prompt')}>
                  {money.compact(row.prompt_tokens)}
                </Field>
                <Field label={t('detail.completion')}>
                  {money.compact(row.completion_tokens)}
                </Field>
                <Field label={t('detail.cachedRead')}>
                  {money.compact(row.cached_tokens)}
                </Field>
                <Field label={t('detail.cacheWrite')}>
                  {money.compact(row.cache_write_tokens)}
                </Field>
                <Field label={t('detail.reasoning')}>
                  {money.compact(row.reasoning_tokens)}
                </Field>
                <Field label={t('detail.total')}>
                  {money.compact(row.total_tokens)}
                </Field>
                <Field label={t('table.cost')}>{money.usd(row.cost_usd)}</Field>
                <Field label={t('table.latency')}>
                  {t('ms', { value: row.latency_ms })}
                </Field>
              </div>
              {row.error && (
                <Field label={t('detail.error')}>
                  <pre className="whitespace-pre-wrap break-words rounded-lg border bg-background p-3 font-mono text-xs text-destructive">
                    {row.error}
                  </pre>
                </Field>
              )}
              <p className="text-xs text-muted-foreground">
                {t('detail.noContent')}
              </p>
            </dl>
          </>
        )}
      </SheetContent>
    </Sheet>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="tabular-nums">{children}</dd>
    </div>
  );
}
