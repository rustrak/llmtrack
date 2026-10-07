# llmtrack

A lightweight, self-hosted LLM gateway. Point any OpenAI SDK at it, hand each
team (or person) a virtual key, and see exactly what every one spent.

- **OpenAI's and Anthropic's APIs** (chat, completions, responses, embeddings,
  images, audio, moderations, `/v1/messages`) in front of OpenAI, Azure,
  Anthropic, Gemini, Groq, Mistral, DeepSeek, OpenRouter, Together, Ollama, or
  any OpenAI-compatible server, translated where the dialects differ.
- **Router**: several deployments per model name with
  weights, retries, cooldowns of failing deployments, and fallbacks to other
  models (also for context-window and content-policy errors).
- **Teams and virtual keys**: team or personal keys; budgets that reset daily,
  weekly or monthly; requests- and tokens-per-minute limits; model access;
  expiry; block, regenerate, revoke. Keys are stored hashed and shown once.
- **Users**: invite by link (no email needed) or add with a password; see
  what each user's keys spent.
- **Spend by customer and tag**: the request's `user`,
  `x-litellm-customer-id`, `metadata.tags` or `x-litellm-tags`.
- **Management API**: everything the dashboard does, with a master
  key or a personal key as a bearer token.
- **Billing**: cost per request from a bundled price list (synced daily) or a model's own
  rates (cache, reasoning and long-context tokens priced apart), rolled up per
  day, team, key and model. Integer money, no float drift.
- **Fast**: a single Rust binary; keys and routes cached in memory, usage
  written in batches off the request path, streams forwarded untouched.
- **SQLite or PostgreSQL**, and a dashboard in the same binary.

## Quick start

```bash
cd apps/server
export SECRET_KEY=$(openssl rand -hex 32)
CREATE_SUPERUSER="admin@example.com:changeme123" cargo run
```

Sign in at <http://localhost:4000> (build the dashboard first with
`pnpm install && pnpm build`), add a model, create a team and a key, then:

```python
from openai import OpenAI
client = OpenAI(base_url="http://localhost:4000/v1", api_key="sk-…")
client.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])
```

See `CLAUDE.md` for the layout and `apps/server/.env.example` for configuration.

## LiteLLM compatibility

Clients written for LiteLLM's proxy work unchanged: llmtrack reads the
`x-litellm-*` headers, `litellm_metadata`, `LITELLM_MASTER_KEY`, `/key/info`
and the `router_settings` shape, and answers with the same `x-litellm-*`
response headers.

## Performance

Measured with `packages/benchmarks` (`bash scripts/compare.sh`): llmtrack and
LiteLLM (`main-stable`, 2 workers, Postgres) in containers with the same limits
(2 CPUs, 2 GB), both authenticating a virtual key and logging spend, both in
front of the same mock provider that answers instantly. "Added" is the latency
on top of calling that provider directly, so it is what the gateway itself
costs. 15 s per run after a 3 s warm-up, Docker Desktop on Apple silicon.

| Scenario | Concurrency | llmtrack added p50 / p99 | LiteLLM added p50 / p99 | llmtrack req/s | LiteLLM req/s |
|---|---:|---:|---:|---:|---:|
| Chat | 1 | +0.18 / +3.03 ms | +3.81 / +15.14 ms | 476 | 167 |
| Chat | 16 | +0.11 / +0.29 ms | +31.87 / +71.88 ms | 10,716 | 455 |
| Chat | 64 | +0.54 / +25.40 ms | +151.43 / +426.31 ms | 13,401 | 413 |
| Stream | 1 | +0.30 / +1.06 ms | +9.25 / +15.26 ms | 418 | 87 |
| Stream | 16 | +1.11 / +9.79 ms | +92.62 / +158.23 ms | 4,374 | 165 |
| Stream | 64 | +3.24 / +49.83 ms | +408.74 / +1,014.82 ms | 8,185 | 140 |

Time to the first byte of a stream, p50: 1.7–2.7 ms through llmtrack, 9.6–296 ms
through LiteLLM. No errors on either side.

| Resources | llmtrack | LiteLLM |
|---|---:|---:|
| Peak memory under load | 39–89 MB | 1.85–2.03 GB |
| Memory after the run | 76 MB | 1.8 GB |

LiteLLM ran close to its 2 GB limit, which may cost it some throughput; rerun
with `GATEWAY_MEMORY=4g LITELLM_WORKERS=4 bash scripts/compare.sh` to give it
more room. A real provider adds hundreds of milliseconds to every request, so
in production the gateway's share is what the "added" columns show, not the
totals. Full tables: `packages/benchmarks/results/REPORT.md`.

## Credits

The price list (`pricing/model_prices.json`) is LiteLLM's
`model_prices_and_context_window.json`, MIT-licensed; see `pricing/NOTICE`.

## License

GPL-3.0. See [LICENSE](LICENSE). Copyright © 2026 Abian Suarez.
