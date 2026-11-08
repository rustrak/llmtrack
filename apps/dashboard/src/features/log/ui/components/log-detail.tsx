import { Tag } from '@rustrak/ui';
import type React from 'react';
import { useTranslations } from 'use-intl';
import type { LogLine } from '@/shared/api/schemas';
import { CopyButton } from '@/shared/ui/components/copy-button';
import { Caption } from '@/shared/ui/components/page';
import { PanelBody, SidePanel } from '@/shared/ui/components/side-panel';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { RequestLogsIcon } from '@/shared/ui/icons';
import { RequestContent } from './request-content';

export function StatusBadge({ code }: { code: number }) {
  const t = useTranslations('logs');
  if (code < 400)
    return (
      <Tag tone="brand">
        <span className="font-mono">{code}</span>
      </Tag>
    );
  return (
    <Tag tone="error">
      {code === 499 ? (
        t('clientLeft')
      ) : (
        <span className="font-mono">{code}</span>
      )}
    </Tag>
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
    <SidePanel
      size="lg"
      open={row !== null}
      onOpenChange={(open) => !open && onClose()}
      icon={RequestLogsIcon}
      title={
        row && (
          <span className="flex min-w-0 items-center gap-2">
            <span className="truncate font-mono">{row.model_name}</span>
            <StatusBadge code={row.status_code} />
          </span>
        )
      }
      description={row && money.dateTime(row.created_at)}
    >
      {row && (
        <PanelBody>
          <dl className="flex flex-col gap-4 text-body text-fg">
            <Field label={t('detail.requestId')}>
              <span className="flex items-center gap-2">
                <code className="truncate font-mono text-mono-sm">
                  {row.request_id}
                </code>
                <CopyButton text={row.request_id} />
              </span>
            </Field>
            <Field label={t('detail.endpoint')}>
              <span className="font-mono text-mono-sm">/v1/{row.endpoint}</span>
              {row.stream && ` · ${t('stream')}`}
            </Field>
            <Field label={t('detail.provider')}>
              {row.provider}
              {row.model_id !== null && (
                <span className="ml-2 text-fg-subtle text-meta">
                  {t('detail.deployment', { id: row.model_id })}
                </span>
              )}
            </Field>
            <Field label={t('table.team')}>
              {billedTo(row, t('personal'))}
            </Field>
            <Field label={t('table.key')}>
              {row.key_name ?? `#${row.key_id}`}{' '}
              <span className="font-mono text-fg-subtle text-mono-sm">
                {row.key_hint}
              </span>
            </Field>
            {row.end_user && (
              <Field label={t('detail.endUser')}>
                <span className="font-mono text-mono-sm">{row.end_user}</span>
              </Field>
            )}
            {row.tags.length > 0 && (
              <Field label={t('detail.tags')}>
                <span className="flex flex-wrap gap-1">
                  {row.tags.map((tag) => (
                    <Tag key={tag} variant="soft">
                      {tag}
                    </Tag>
                  ))}
                </span>
              </Field>
            )}
            <div className="grid grid-cols-2 gap-x-4 gap-y-3 rounded-xl border border-border-subtle bg-surface-raised p-4 font-mono text-mono">
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
                <pre className="whitespace-pre-wrap break-words rounded-lg border border-danger/40 bg-danger-surface p-3 font-mono text-danger-fg text-mono-sm">
                  {row.error}
                </pre>
              </Field>
            )}
            <Field label={t('detail.content')}>
              {row.has_body ? (
                <RequestContent requestId={row.request_id} />
              ) : (
                <p className="text-fg-subtle text-meta">
                  {t('detail.noContent')}
                </p>
              )}
            </Field>
          </dl>
        </PanelBody>
      )}
    </SidePanel>
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
    <div className="flex min-w-0 flex-col gap-1">
      <dt>
        <Caption>{label}</Caption>
      </dt>
      <dd className="min-w-0 tabular-nums">{children}</dd>
    </div>
  );
}
