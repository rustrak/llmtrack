# @llmtrack/server

## 0.3.0

### Minor Changes

- [`15a78de`](https://github.com/rustrak/llmtrack/commit/15a78de8d3aff4a1e799e07d4befa78c30394b98) Thanks [@AbianS](https://github.com/AbianS)! - llmtrack now has an MCP server at `/mcp`, off until an admin turns it on in Settings → AI assistants. Assistants such as Claude Code or claude.ai connect by signing in to llmtrack (OAuth, with a consent page), which needs the public URL set in Settings; each user sees and disconnects their connected assistants on the same page. Through it an assistant reads and creates teams, keys, models, people, labels and invitations with its user's permissions, answers spend questions, filters the request log, reads a request's full conversation, and searches names and conversations in one call. Stored request and response bodies are now indexed for full-text search, so the Logs page and the API (`/api/logs?text=`) find requests by the words said in them. Usage, logs and the PDF and Excel reports filter by several teams, keys, models, people, labels, customers and tags at once, and any of them can be left out instead, such as a team without one of its keys. The dashboard's filters can now select several values and switch between including and excluding them. Dependencies with security advisories are updated. The bundled price list is refreshed.

## 0.2.1

### Patch Changes

- [`5c64c07`](https://github.com/rustrak/llmtrack/commit/5c64c07269a8b7615464d1bdf18bc648ebf6d962) Thanks [@AbianS](https://github.com/AbianS)! - Keys can now keep request and response bodies, off by default. Turn it on per key and set how many days to keep them (empty keeps them until deleted); streamed replies are folded into one, inline images and files are stored as their size only, and an hourly task purges expired bodies and those of deleted keys. Bodies live apart from usage and billing. In the dashboard, Logs shows each request's content and exports the filtered requests as JSON Lines, either every field with both bodies or as an OpenAI chat dataset for fine-tuning and evals. The bundled price list is refreshed.

## 0.2.0

### Minor Changes

- [`55b820d`](https://github.com/rustrak/llmtrack/commit/55b820d8e7585ee80fbea47cbb986dcb9a251d24) Thanks [@AbianS](https://github.com/AbianS)! - Add customizable PDF and Excel usage reports. Add labels for virtual keys and filter usage and exported reports by label.

## 0.1.0

### Minor Changes

- [`416c5d6`](https://github.com/rustrak/llmtrack/commit/416c5d6e7c9601f8efb8ace3b970b2c15a0da38e) Thanks [@AbianS](https://github.com/AbianS)! - First public release of llmtrack: a self-hosted LLM gateway in one Rust binary. OpenAI and Anthropic APIs in front of OpenAI, Azure, Anthropic, Gemini, Groq, Mistral, DeepSeek, OpenRouter, Together, Ollama and any OpenAI-compatible server; a router with weighted deployments, retries, cooldowns and fallbacks; teams, people and virtual keys with budgets, rate limits and model access; spend per request, team, key, person, customer and tag, priced from a daily-synced price list or a model's own rates; a management API and a dashboard in English and Spanish; SQLite or PostgreSQL.
