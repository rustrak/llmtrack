/** What the Playground reads off a chat completion's usage. */
export interface ChatUsage {
  prompt_tokens: number;
  completion_tokens: number;
}

/**
 * Splits a growing server-sent-events buffer into the `data:` payloads of
 * its complete events, and the unfinished tail to keep for the next read.
 * Comments and OpenAI's `[DONE]` are dropped.
 */
export function takeEvents(buffer: string): { events: string[]; rest: string } {
  const parts = buffer.replace(/\r\n/g, '\n').split('\n\n');
  const rest = parts.pop() ?? '';
  const events = parts
    .map((event) =>
      event
        .split('\n')
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trim())
        .join('\n'),
    )
    .filter((data) => data !== '' && data !== '[DONE]');
  return { events, rest };
}

/** The text one chunk adds, and the usage when it is the last one. */
export function readChunk(chunk: {
  choices?: { delta?: { content?: string | null } }[];
  usage?: ChatUsage | null;
}): { text: string; usage: ChatUsage | undefined } {
  return {
    text: chunk.choices?.[0]?.delta?.content ?? '',
    usage: chunk.usage ?? undefined,
  };
}
