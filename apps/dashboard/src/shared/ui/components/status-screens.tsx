import { Link } from '@tanstack/react-router';
import { AlertTriangle } from 'lucide-react';
import { useTranslations } from 'use-intl';
import type { ApiError } from '@/shared/api/http';
import { Button } from './shadcn/button';

/** The server did not answer: say so, instead of pretending to be logged out. */
export function OutageScreen({ error }: { error: ApiError }) {
  const t = useTranslations('errors');
  return (
    <div className="flex flex-1 items-center justify-center px-6 py-20">
      <div className="max-w-md space-y-4 text-center">
        <AlertTriangle className="mx-auto size-6 text-destructive" />
        <h1 className="text-xl font-semibold">{t('outageTitle')}</h1>
        <p className="text-sm text-muted-foreground">{error.message}</p>
        <Button onClick={() => window.location.reload()}>
          {t('tryAgain')}
        </Button>
      </div>
    </div>
  );
}

export function ErrorScreen({ error }: { error: unknown }) {
  const t = useTranslations('errors');
  return (
    <div className="flex flex-1 items-center justify-center px-6 py-20">
      <div className="max-w-md space-y-4 text-center">
        <AlertTriangle className="mx-auto size-6 text-destructive" />
        <h1 className="text-xl font-semibold">{t('somethingWrong')}</h1>
        <p className="text-sm text-muted-foreground">
          {error instanceof Error ? error.message : String(error)}
        </p>
        <Button onClick={() => window.location.reload()}>{t('reload')}</Button>
      </div>
    </div>
  );
}

export function NotFoundScreen() {
  const t = useTranslations('errors');
  return (
    <div className="flex flex-1 items-center justify-center px-6 py-20">
      <div className="space-y-4 text-center">
        <p className="font-mono text-5xl font-bold text-primary">404</p>
        <p className="text-sm text-muted-foreground">{t('notFound')}</p>
        <Button nativeButton={false} render={<Link to="/" />}>
          {t('goHome')}
        </Button>
      </div>
    </div>
  );
}
