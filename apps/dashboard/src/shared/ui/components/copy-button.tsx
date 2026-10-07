import { Check, Copy } from 'lucide-react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { copyToClipboard } from '@/shared/lib/clipboard';
import { Button } from './shadcn/button';

export function CopyButton({ text }: { text: string }) {
  const t = useTranslations('common');
  const [copied, setCopied] = useState(false);
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={async () => {
        if (!(await copyToClipboard(text))) return;
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }}
    >
      {copied ? <Check /> : <Copy />}
      {copied ? t('copied') : t('copy')}
    </Button>
  );
}

export function CodeBlock({ code }: { code: string }) {
  return (
    <div className="relative">
      <pre className="overflow-x-auto rounded-lg border bg-background p-4 pr-24 font-mono text-xs leading-relaxed">
        {code}
      </pre>
      <div className="absolute top-2 right-2">
        <CopyButton text={code} />
      </div>
    </div>
  );
}
