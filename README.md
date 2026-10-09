<div align="center">

<img width="2000" height="1080" alt="hero" src="https://github.com/user-attachments/assets/e67f93f1-02c9-4e66-84d8-1c2e74765b40" />

[![CI](https://github.com/rustrak/llmtrack/actions/workflows/ci.yml/badge.svg)](https://github.com/rustrak/llmtrack/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rustrak/llmtrack)](https://github.com/rustrak/llmtrack/releases)
[![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)
[![Docker pulls](https://img.shields.io/docker/pulls/rustrak/llmtrack)](https://hub.docker.com/r/rustrak/llmtrack)
[![Sponsors](https://img.shields.io/github/sponsors/AbianS?label=sponsors&color=db61a2)](https://github.com/sponsors/AbianS)

[Quick start](#quick-start) ·
[Performance](#performance) ·
[LiteLLM compatibility](#coming-from-litellm) ·
[Report a bug](https://github.com/rustrak/llmtrack/issues)

</div>

llmtrack is a self-hosted LLM gateway. Point any OpenAI SDK at it, hand each
team (or person) a virtual key, and see exactly what every one of them spent.

It speaks OpenAI's and Anthropic's APIs and sits in front of OpenAI, Azure,
Anthropic, Gemini, Groq, Mistral, DeepSeek, OpenRouter, Together, Ollama, or
any OpenAI-compatible server, translating where the dialects differ. It is one
Rust binary with the dashboard inside, on SQLite by default, so the whole
install is one container and nothing to provision.

<img width="2000" height="1000" alt="wp-usage" src="https://github.com/user-attachments/assets/ce3dc8f1-952a-4774-887f-58c8a15816f8" />

## Why llmtrack

Once more than one team calls an LLM, two questions arrive at the same time:
who spent this, and why is the provider key in six different `.env` files. A
gateway answers both, but the usual one is a Python service that wants its own
Postgres, its own Redis and two gigabytes of memory before it has proxied a
single token.

llmtrack is the same idea built the way [Rustrak](https://github.com/rustrak/rustrak)
is: one small process that is fast enough to forget about. Keys and routes are
cached in memory, usage is written in batches off the request path, and streams
are forwarded untouched, so the gateway adds a fraction of a millisecond to a
request that is about to spend hundreds of them at the provider.

<img width="2000" height="820" alt="numbers" src="https://github.com/user-attachments/assets/0fd18b49-9cb0-454f-9673-2785f31fe8f3" />

Provider keys live in one place, encrypted. Applications only ever hold a
virtual key that you can limit, block or revoke without touching the provider.

## Quick start

```bash
docker run -d --name llmtrack -p 4000:4000 -v llmtrack_data:/data \
  -e SECRET_KEY=$(openssl rand -hex 32) \
  -e CREATE_SUPERUSER=admin@example.com:changeme123 \
  rustrak/llmtrack:latest
```

Open <http://localhost:4000> and sign in with those credentials. Add a model,
create a team and a key, then call it like OpenAI:

```python
from openai import OpenAI

client = OpenAI(base_url="http://localhost:4000/v1", api_key="sk-…")
client.chat.completions.create(
    model="gpt-5.6",
    messages=[{"role": "user", "content": "Hello"}],
)
```

`SECRET_KEY` signs the session and encrypts the provider keys stored in the
database, so keep it: changing it makes every stored provider key unreadable.
For PostgreSQL, set `DATABASE_URL=postgres://…` on the `:postgres` image. Every
variable is in [`apps/server/.env.example`](apps/server/.env.example).

From source:

```bash
cd apps/server
export SECRET_KEY=$(openssl rand -hex 32)
CREATE_SUPERUSER="admin@example.com:changeme123" cargo run
```

(`pnpm install && pnpm build` first if you want the dashboard; without it
`cargo build` still gives you the whole API.)

## Coming from LiteLLM

<img width="2000" height="700" alt="base-url" src="https://github.com/user-attachments/assets/970a44de-f8cc-4aed-a9e2-3a6bc59c2371" />

Clients written for LiteLLM's proxy work unchanged. llmtrack reads the
`x-litellm-*` headers, `litellm_metadata`, `LITELLM_MASTER_KEY`, `/key/info` and
the `router_settings` shape, and answers with the same `x-litellm-*` response
headers. The price list is LiteLLM's own, synced daily, so a request costs the
same on both.

## What's inside

### Usage and billing

Every request is priced as it happens: from the bundled price list (synced
daily) or from a model's own rates, with cache reads, cache writes, reasoning
tokens and long-context tiers billed apart. Money is integers end to end, so
the sum of a month never drifts from the sum of its days.

The usage page splits spend, requests or tokens per day by **model, team, key,
person, customer or tag**, compares the range with the one before it, and
exports to CSV. Customers and tags come from the request itself: the OpenAI
`user` field or `x-litellm-customer-id`, and `metadata.tags` or
`x-litellm-tags`.

```python
client.chat.completions.create(
    model="claude-sonnet-5",
    messages=messages,
    user="acme-corp",                                   # billed to this customer
    extra_body={"metadata": {"tags": ["feature:triage"]}},
)
```

### Teams and virtual keys

A key bills to a team, or to you for a personal key. Each one, and each team,
can carry a budget that resets daily, weekly or monthly, requests- and
tokens-per-minute limits, a list of models it may call and an expiry date. Keys
can be blocked, regenerated and revoked; they are stored hashed and shown once.

Teams also have **people**: someone who spends through a team's keys without a
dashboard account, so "what did Lena's Cursor cost this month" has an answer.

<img width="2000" height="800" alt="wp-keys" src="https://github.com/user-attachments/assets/9fdcf5ce-cb5f-4618-b83e-96e68c742404" />

### Router

One public model name can point at several deployments, say `gpt-5.6` on
OpenAI and on Azure, with weights. A failing deployment is retried, then put in
cooldown so traffic stops landing on it, and a model that keeps failing falls
back to another model. Fallbacks also apply to context-window and
content-policy errors, so a prompt too long for one model can go to one that
takes it.

<img width="2000" height="840" alt="wp-models" src="https://github.com/user-attachments/assets/2e1c9ffb-a9c1-48f6-bdd2-99ea406a9639" />


### APIs

| | |
|---|---|
| OpenAI | chat, completions, responses, embeddings, images, audio, moderations |
| Anthropic | `/v1/messages` |
| Providers | OpenAI, Azure OpenAI, Anthropic, Gemini, Groq, Mistral, DeepSeek, OpenRouter, Together, Ollama, any OpenAI-compatible server |

### Logs

Every request, with who sent it, which deployment answered, its tokens, cost
and latency. Failures keep the provider's error. **Prompts and completions are
not stored** unless a key opts in: then each request and its reply (a stream
folded into one) can be read in the log, kept for as many days as the key says,
and exported as JSON Lines, all fields or as an OpenAI chat dataset.

<img width="2000" height="800" alt="wp-logs" src="https://github.com/user-attachments/assets/7926336a-8031-4527-8e06-8691acf89927" />

### Users and the management API

Invite people by link (no email server needed) or add them with a password, and
see what each user's keys spent. Everything the dashboard does is in the
management API, with the master key or a personal key as a bearer token, so a
CI job can create the key for a new service. The dashboard is in English and
Spanish, and there is a playground for checking a model answers through a real
key.

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
totals. Full tables: [`packages/benchmarks/results/REPORT.md`](packages/benchmarks/results/REPORT.md).

## Contributing

`CLAUDE.md` describes the layout and every command CI runs (`pnpm run ci`).
Server changes are written test-first against a real database and a fake
provider; every sentence in the dashboard lives in both `en.json` and `es.json`.

## Credits

The price list (`pricing/model_prices.json`) is LiteLLM's
`model_prices_and_context_window.json`, MIT-licensed; see `pricing/NOTICE`.

## License

GPL-3.0. See [LICENSE](LICENSE). Copyright © 2026 AbianS.
