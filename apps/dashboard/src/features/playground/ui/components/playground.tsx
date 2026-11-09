import {
  ArrowRightIcon,
  Button,
  cn,
  DeleteIcon,
  Field,
  FieldHint,
  FieldLabel,
  Input,
  MuteIcon,
  Text,
  Textarea,
} from '@rustrak/ui';
import { useEffect, useRef, useState } from 'react';
import { useTranslations } from 'use-intl';
import type { ApiKey } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
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
    return (
      <Text render={<p />} tone="subtle">
        {t('noKeys')}
      </Text>
    );
  }

  return (
    <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[18rem_1fr]">
      <aside className="flex flex-col gap-4">
        <Field>
          <FieldLabel>{t('key')}</FieldLabel>
          <Choice
            aria-label={t('key')}
            value={keyId}
            onChange={setKeyId}
            options={keys.map((k) => ({
              value: String(k.id),
              label: `${k.name} · ${k.team_name ?? t('personal')}`,
            }))}
          />
          <FieldHint>{t('keyHint')}</FieldHint>
        </Field>
        <Field>
          <FieldLabel>{t('model')}</FieldLabel>
          <Choice
            aria-label={t('model')}
            value={model}
            onChange={setModel}
            placeholder={t('noModels')}
            disabled={models.length === 0}
            options={models.map((name) => ({ value: name, label: name }))}
          />
        </Field>
        <Field>
          <FieldLabel>{t('system')}</FieldLabel>
          <Textarea
            rows={4}
            className="max-h-48"
            placeholder={t('systemPlaceholder')}
            value={system}
            onChange={(event) => setSystem(event.target.value)}
          />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field>
            <FieldLabel>{t('temperature')}</FieldLabel>
            <Input
              className="font-mono"
              inputMode="decimal"
              placeholder="1"
              value={temperature}
              onChange={(event) => setTemperature(event.target.value)}
            />
          </Field>
          <Field>
            <FieldLabel>{t('maxTokens')}</FieldLabel>
            <Input
              className="font-mono"
              inputMode="numeric"
              placeholder={t('default')}
              value={maxTokens}
              onChange={(event) => setMaxTokens(event.target.value)}
            />
          </Field>
        </div>
      </aside>

      <section className="flex min-h-[28rem] flex-col overflow-hidden rounded-2xl border border-border-subtle bg-surface">
        <div className="flex h-row-head items-center justify-between border-border-divider border-b px-4">
          <span className="truncate font-mono text-mono-sm text-fg-secondary">
            {model || t('noModels')}
          </span>
          <Button
            variant="ghost"
            size="sm"
            icon={DeleteIcon}
            disabled={messages.length === 0 || running !== null}
            onClick={() => setMessages([])}
          >
            {t('clear')}
          </Button>
        </div>
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
          {messages.length === 0 && (
            <p className="py-12 text-center text-body text-fg-subtle">
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
          className="flex items-end gap-2 border-border-divider border-t p-3"
          onSubmit={(event) => {
            event.preventDefault();
            send();
          }}
        >
          <Textarea
            aria-label={t('message')}
            rows={1}
            boxClassName="flex-1"
            className="max-h-48"
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
              icon={MuteIcon}
              onClick={() => running.abort()}
            >
              {t('stop')}
            </Button>
          ) : (
            <Button
              type="submit"
              variant="primary"
              icon={ArrowRightIcon}
              disabled={!draft.trim() || !model}
            >
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
          'max-w-[85%] whitespace-pre-wrap rounded-lg px-3 py-2 text-body',
          mine
            ? 'bg-surface-brand text-fg-on-brand'
            : 'bg-surface-chip text-fg',
        )}
      >
        {message.content || (pending ? '…' : '')}
      </div>
      {message.error && (
        <p className="text-hint text-danger-fg">{message.error}</p>
      )}
      {reply && (
        <p className="font-mono text-mono-sm text-fg-meta tabular-nums">
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
