import { Eraser, Send, Square } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslations } from 'use-intl';
import type { ApiKey } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { cn } from '@/shared/lib/utils';
import { Choice } from '@/shared/ui/components/choice';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import { Textarea } from '@/shared/ui/components/shadcn/textarea';
import { useMoney } from '@/shared/ui/hooks/use-money';
import {
  type ChatReply,
  listKeyModels,
  priceReply,
  streamChat,
} from '../../api/chat';

interface Message {
  role: 'user' | 'assistant';
  content: string;
  /** Assistant replies, once finished. */
  reply?: ChatReply & { costUsd?: number };
  error?: string;
}

/**
 * A chat with any model a key may call, to see that it answers: streamed,
 * with what each reply took and cost. Billed to the key like real traffic.
 */
export function Playground({ keys }: { keys: ApiKey[] }) {
  const t = useTranslations('playground');
  const [keyId, setKeyId] = useState(keys[0] ? String(keys[0].id) : '');
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState('');
  const [system, setSystem] = useState('');
  const [temperature, setTemperature] = useState('');
  const [maxTokens, setMaxTokens] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState('');
  const [running, setRunning] = useState<AbortController | null>(null);
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!keyId) return;
    let current = true;
    listKeyModels(Number(keyId)).then((result) => {
      if (!current) return;
      const names = result.success ? result.data : [];
      setModels(names);
      setModel((m) => (names.includes(m) ? m : (names[0] ?? '')));
    });
    return () => {
      current = false;
    };
  }, [keyId]);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: 'end' });
  }, [messages]);

  const updateLast = (change: (last: Message) => Message) =>
    setMessages((all) => [...all.slice(0, -1), change(all[all.length - 1])]);

  const chatBody = (history: Pick<Message, 'role' | 'content'>[]) => ({
    key_id: Number(keyId),
    model,
    messages: [
      ...(system.trim() ? [{ role: 'system', content: system }] : []),
      ...history.map(({ role, content }) => ({ role, content })),
    ],
    ...(temperature ? { temperature: Number(temperature) } : {}),
    ...(maxTokens ? { max_tokens: Number(maxTokens) } : {}),
  });

  const send = async () => {
    const text = draft.trim();
    if (!text || !model || running) return;
    const history = [...messages, { role: 'user' as const, content: text }];
    setMessages([...history, { role: 'assistant', content: '' }]);
    setDraft('');
    const controller = new AbortController();
    setRunning(controller);
    const result = await streamChat(
      chatBody(history),
      (chunk) =>
        updateLast((last) => ({ ...last, content: last.content + chunk })),
      controller.signal,
    );
    setRunning(null);
    if (!result.success) {
      updateLast((last) => ({ ...last, error: readableMessage(result.error) }));
      return;
    }
    updateLast((last) => ({ ...last, reply: result.data }));
    const costUsd = await priced(result.data);
    if (costUsd !== undefined) {
      updateLast((last) => ({ ...last, reply: { ...result.data, costUsd } }));
    }
  };

  if (keys.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('noKeys')}</p>;
  }

  return (
    <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[18rem_1fr]">
      <aside className="space-y-4">
        <div className="space-y-1.5">
          <Label>{t('key')}</Label>
          <Choice
            aria-label={t('key')}
            value={keyId}
            onChange={setKeyId}
            options={keys.map((k) => ({
              value: String(k.id),
              label: `${k.name} · ${k.team_name ?? t('personal')}`,
            }))}
          />
          <p className="text-xs text-muted-foreground">{t('keyHint')}</p>
        </div>
        <div className="space-y-1.5">
          <Label>{t('model')}</Label>
          <Choice
            aria-label={t('model')}
            value={model}
            onChange={setModel}
            placeholder={t('noModels')}
            disabled={models.length === 0}
            options={models.map((name) => ({ value: name, label: name }))}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="pg-system">{t('system')}</Label>
          <Textarea
            id="pg-system"
            className="max-h-48"
            placeholder={t('systemPlaceholder')}
            value={system}
            onChange={(event) => setSystem(event.target.value)}
          />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1.5">
            <Label htmlFor="pg-temperature">{t('temperature')}</Label>
            <Input
              id="pg-temperature"
              inputMode="decimal"
              placeholder="1"
              value={temperature}
              onChange={(event) => setTemperature(event.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="pg-max-tokens">{t('maxTokens')}</Label>
            <Input
              id="pg-max-tokens"
              inputMode="numeric"
              placeholder={t('default')}
              value={maxTokens}
              onChange={(event) => setMaxTokens(event.target.value)}
            />
          </div>
        </div>
      </aside>

      <section className="flex min-h-[28rem] flex-col rounded-xl border bg-card">
        <div className="flex items-center justify-between border-b px-4 py-2">
          <span className="text-sm text-muted-foreground">
            {model || t('noModels')}
          </span>
          <Button
            variant="ghost"
            size="sm"
            disabled={messages.length === 0 || running !== null}
            onClick={() => setMessages([])}
          >
            <Eraser />
            {t('clear')}
          </Button>
        </div>
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
          {messages.length === 0 && (
            <p className="py-12 text-center text-sm text-muted-foreground">
              {t('empty')}
            </p>
          )}
          {messages.map((message, i) => (
            <MessageBubble
              key={i}
              message={message}
              pending={running !== null && i === messages.length - 1}
            />
          ))}
          <div ref={bottom} />
        </div>
        <form
          className="flex items-end gap-2 border-t p-3"
          onSubmit={(event) => {
            event.preventDefault();
            send();
          }}
        >
          <Textarea
            aria-label={t('message')}
            className="max-h-48 min-h-10"
            placeholder={t('messagePlaceholder')}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                send();
              }
            }}
          />
          {running ? (
            <Button
              type="button"
              variant="outline"
              onClick={() => running.abort()}
            >
              <Square />
              {t('stop')}
            </Button>
          ) : (
            <Button type="submit" disabled={!draft.trim() || !model}>
              <Send />
              {t('send')}
            </Button>
          )}
        </form>
      </section>
    </div>
  );
}

/** What a reply cost, once it is known which deployment answered. */
async function priced(reply: ChatReply): Promise<number | undefined> {
  if (!reply.modelId || !reply.usage) return undefined;
  const cost = await priceReply(reply.modelId, reply.usage);
  return cost.success ? cost.data.cost_usd : undefined;
}

function MessageBubble({
  message,
  pending,
}: {
  message: Message;
  pending: boolean;
}) {
  const t = useTranslations('playground');
  const money = useMoney();
  const mine = message.role === 'user';
  const { reply } = message;
  const ms = (value: number) => t('ms', { value: Math.round(value) });
  return (
    <div className={cn('flex flex-col gap-1', mine && 'items-end')}>
      <div
        className={cn(
          'max-w-[85%] whitespace-pre-wrap rounded-lg px-3 py-2 text-sm',
          mine ? 'bg-primary text-primary-foreground' : 'bg-muted',
        )}
      >
        {message.content || (pending ? '…' : '')}
      </div>
      {message.error && (
        <p className="text-xs text-destructive">{message.error}</p>
      )}
      {reply && (
        <p className="text-xs text-muted-foreground tabular-nums">
          {[
            reply.ttftMs !== null && t('ttft', { value: ms(reply.ttftMs) }),
            t('total', { value: ms(reply.totalMs) }),
            reply.usage &&
              t('tokens', {
                input: reply.usage.prompt_tokens,
                output: reply.usage.completion_tokens,
              }),
            reply.costUsd !== undefined && money.usd(reply.costUsd),
          ]
            .filter(Boolean)
            .join(' · ')}
        </p>
      )}
    </div>
  );
}
