import { useEffect, useState } from 'react';
import { useTranslations } from 'use-intl';
import type { Body } from '@/shared/api/schemas';
import { CodeBlock } from '@/shared/ui/components/copy-button';
import { Caption } from '@/shared/ui/components/page';
import { getBody } from '../../api/queries';

/** What a key that keeps bodies kept of one request, as the JSON that went by. */
export function RequestContent({ requestId }: { requestId: string }) {
  const t = useTranslations('logs.content');
  const [body, setBody] = useState<Body | 'loading' | 'failed'>('loading');

  useEffect(() => {
    let current = true;
    setBody('loading');
    getBody(requestId).then((result) => {
      if (current) setBody(result.success ? result.data : 'failed');
    });
    return () => {
      current = false;
    };
  }, [requestId]);

  if (body === 'loading') {
    return <p className="text-fg-subtle text-meta">{t('loading')}</p>;
  }
  if (body === 'failed') {
    return <p className="text-danger-fg text-hint">{t('failed')}</p>;
  }
  return (
    <div className="flex flex-col gap-2">
      <Caption>{t('request')}</Caption>
      <CodeBlock code={JSON.stringify(body.request, null, 2)} />
      <span className="pt-1">
        <Caption>{t('response')}</Caption>
      </span>
      {body.response === null ? (
        <p className="text-fg-subtle text-meta">{t('noResponse')}</p>
      ) : (
        <CodeBlock code={JSON.stringify(body.response, null, 2)} />
      )}
    </div>
  );
}
