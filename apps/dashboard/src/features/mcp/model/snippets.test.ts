import { describe, expect, it } from 'vitest';
import { mcpSnippet } from './snippets';

const URL = 'https://llm.example.com/mcp';

describe('mcpSnippet', () => {
  it('gives every client the URL and no credential', () => {
    for (const client of ['claudeCode', 'json', 'vscode'] as const) {
      const code = mcpSnippet(client, URL);
      expect(code).toContain(URL);
      expect(code).not.toContain('Authorization');
    }
  });

  it('adds the server to Claude Code over HTTP', () => {
    expect(mcpSnippet('claudeCode', URL)).toBe(
      `claude mcp add --transport http llmtrack ${URL}`,
    );
  });

  it('declares the transport, which Claude Code reads a URL without as stdio', () => {
    expect(JSON.parse(mcpSnippet('json', URL))).toEqual({
      mcpServers: { llmtrack: { type: 'http', url: URL } },
    });
  });

  it("uses VS Code's own `servers` shape for VS Code", () => {
    expect(JSON.parse(mcpSnippet('vscode', URL))).toEqual({
      servers: { llmtrack: { type: 'http', url: URL } },
    });
  });
});
