import { Button, RefreshIcon, WarningIcon } from '@rustrak/ui';
import { Link } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import { useTranslations } from 'use-intl';
import type { ApiError } from '@/shared/api/http';
import { LlmtrackWordmark } from './llmtrack-wordmark';

/**
 * A whole-viewport answer for when there is no page to show, with the mark
 * on top since the sidebar may not be there to carry it.
 */
function StatusScreen({
  mark,
  title,
  description,
  action,
}: {
  mark: ReactNode;
  title: string;
  description?: string;
  action: ReactNode;
}) {
  return (
    <div className="flex min-h-full flex-1 flex-col items-center justify-center gap-10 bg-canvas px-6 py-20">
      <LlmtrackWordmark />
      <div className="flex max-w-md flex-col items-center gap-4 text-center">
        {mark}
        <div className="space-y-2">
          <h1 className="text-title text-fg">{title}</h1>
          {description && (
            <p className="text-body text-fg-subtle">{description}</p>
          )}
        </div>
        <div className="mt-2">{action}</div>
      </div>
    </div>
  );
}

function Warning() {
  return (
    <span className="flex size-12 items-center justify-center rounded-pill bg-danger-surface">
      <WarningIcon size="xl" className="text-danger-fg" />
    </span>
  );
}

/** The server did not answer: say so, instead of pretending to be logged out. */
export function OutageScreen({ error }: { error: ApiError }) {
  const t = useTranslations('errors');
  return (
    <StatusScreen
      mark={<Warning />}
      title={t('outageTitle')}
      description={error.message}
      action={
        <Button
          variant="primary"
          icon={RefreshIcon}
          onClick={() => window.location.reload()}
        >
          {t('tryAgain')}
        </Button>
      }
    />
  );
}

export function ErrorScreen({ error }: { error: unknown }) {
  const t = useTranslations('errors');
  return (
    <StatusScreen
      mark={<Warning />}
      title={t('somethingWrong')}
      description={error instanceof Error ? error.message : String(error)}
      action={
        <Button
          variant="primary"
          icon={RefreshIcon}
          onClick={() => window.location.reload()}
        >
          {t('reload')}
        </Button>
      }
    />
  );
}

export function NotFoundScreen() {
  const t = useTranslations('errors');
  return (
    <StatusScreen
      mark={<p className="font-mono text-numeric-lg text-fg-brand">404</p>}
      title={t('notFound')}
      action={
        <Button variant="primary" render={<Link to="/" />}>
          {t('goHome')}
        </Button>
      }
    />
  );
}
