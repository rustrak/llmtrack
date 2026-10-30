/**
 * The configuration each client needs to reach `/mcp`. Signing in (OAuth)
 * needs nothing but the URL: the client finds the rest from the server and
 * asks the user to approve.
 */
export type SnippetClient = 'claudeCode' | 'json' | 'vscode';

export function mcpSnippet(client: SnippetClient, url: string): string {
  if (client === 'claudeCode') {
    return `claude mcp add --transport http llmtrack ${url}`;
  }
  // `type` because a client that reads a URL without it (Claude Code's
  // `.mcp.json`) takes the entry for a local stdio server.
  const server = { type: 'http', url };
  const config =
    client === 'vscode'
      ? { servers: { llmtrack: server } }
      : { mcpServers: { llmtrack: server } };
  return JSON.stringify(config, null, 2);
}
