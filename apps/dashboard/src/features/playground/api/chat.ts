import { z } from 'zod';
import { errorFrom, type Result, request } from '@/shared/api/http';
import { type ChatUsage, readChunk, takeEvents } from '../model/sse';

/** The model names a key may call. */
export const listKeyModels = (keyId: number) =>
  request('GET', `/api/playground/models?key_id=${keyId}`, z.array(z.string()));

/** A reply's cost, priced by the deployment that answered it. */
export const priceReply = (modelId: number, usage: ChatUsage) =>
  request('POST', '/api/playground/cost', z.object({ cost_usd: z.number() }), {
    model_id: modelId,
    usage,
  });

export interface ChatReply {
  /** The deployment that answered (`x-litellm-model-id`). */
  modelId: number | null;
  usage?: ChatUsage;
  /** Time to the first token; `null` when none came. */
  ttftMs: number | null;
  totalMs: number;
}

const failure = (message: string): Result<ChatReply> => ({
  success: false,
  error: { kind: 'network', status: 0, message, fields: [] },
});

/**
 * Sends a chat through the Playground and streams the reply's text into
 * `onText`. Stopping (`signal`) keeps what arrived so far.
 */
export async function streamChat(
  body: Record<string, unknown>,
  onText: (text: string) => void,
  signal: AbortSignal,
): Promise<Result<ChatReply>> {
  const started = performance.now();
  let response: Response;
  try {
    response = await fetch('/api/playground/chat', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        ...body,
        stream: true,
        stream_options: { include_usage: true },
      }),
      signal,
    });
  } catch (error) {
    return failure(String(error));
  }
  if (!response.ok || !response.body) {
    return { success: false, error: await errorFrom(response) };
  }
  const reply: ChatReply = {
    modelId: Number(response.headers.get('x-litellm-model-id')) || null,
    ttftMs: null,
    totalMs: 0,
  };
  const error = await drain(response.body, (text, usage) => {
    if (text) {
      reply.ttftMs ??= performance.now() - started;
      onText(text);
    }
    if (usage) reply.usage = usage;
  }).catch((e) => (signal.aborted ? null : String(e)));
  if (error) return failure(error);
  reply.totalMs = performance.now() - started;
  return { success: true, data: reply };
}

/** Reads a stream to its end; the provider's error, if one came instead. */
async function drain(
  stream: ReadableStream<Uint8Array>,
  onChunk: (text: string, usage: ChatUsage | undefined) => void,
): Promise<string | null> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  for (;;) {
    const { done, value } = await reader.read();
    if (done) return null;
    const { events, rest } = takeEvents(
      buffer + decoder.decode(value, { stream: true }),
    );
    buffer = rest;
    for (const data of events) {
      const chunk = JSON.parse(data);
      if (chunk.error) return chunk.error.message ?? data;
      const { text, usage } = readChunk(chunk);
      onChunk(text, usage);
    }
  }
}
