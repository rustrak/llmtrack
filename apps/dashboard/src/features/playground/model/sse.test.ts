import { describe, expect, it } from 'vitest';
import { readChunk, takeEvents } from './sse';

describe('takeEvents', () => {
  it('returns whole events and keeps the unfinished tail', () => {
    expect(takeEvents('data: {"a":1}\n\ndata: {"b"')).toEqual({
      events: ['{"a":1}'],
      rest: 'data: {"b"',
    });
  });

  it('reads CRLF streams and skips comments and [DONE]', () => {
    expect(
      takeEvents(': keep-alive\r\n\r\ndata: {"a":1}\r\n\r\ndata: [DONE]\n\n'),
    ).toEqual({ events: ['{"a":1}'], rest: '' });
  });
});

describe('readChunk', () => {
  it('takes the text delta and, last, the usage', () => {
    expect(readChunk({ choices: [{ delta: { content: 'Hola' } }] })).toEqual({
      text: 'Hola',
      usage: undefined,
    });
    const usage = { prompt_tokens: 3, completion_tokens: 5 };
    expect(readChunk({ choices: [], usage })).toEqual({ text: '', usage });
  });
});
