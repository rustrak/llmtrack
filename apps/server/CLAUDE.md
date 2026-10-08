# llmtrack server

Actix-web 4, SQLx against SQLite (default) or PostgreSQL (feature `postgres`),
reqwest for providers. Root context: `/CLAUDE.md`.

## Layout

```
src/
├── main.rs            entry: config, pool, migrations, superuser, HttpServer
├── net.rs             listening sockets: IPv6 and IPv4 unless HOST says otherwise
├── app.rs             AppState and route wiring, shared by main and the tests
├── config.rs          env config (read through a lookup fn, so tests never touch env)
├── error.rs           AppError → OpenAI-shaped JSON errors, incident ids on 5xx
├── db.rs              pool, migrations, the few SQL fragments the backends spell differently
├── crypto.rs          virtual keys (sk- + SHA-256) and AES-GCM for provider secrets
├── auth.rs            dashboard session extractors: CurrentUser, AdminUser
├── models/            domain types, request/response shapes, money conversions
├── services/          queries and rules: users, invitations, teams, models, keys, usage, access
│   └── report/        usage reports as PDF and XLSX (imprenta), in the reader's language and currency
├── routes/            HTTP handlers, one module per resource; proxy.rs is /v1
├── assets/            the price list (gzip), compiled into the binary
└── gateway/           the hot path
    ├── mod.rs         Gateway: cached keys, routes and catalog; live spend counters
    ├── forward.rs     one path for every endpoint: provider call, metered stream, usage
    ├── anthropic.rs   chat completions → Anthropic Messages (thinking, JSON mode, cache_control…)
    ├── messages.rs    Anthropic Messages → chat completions, for /v1/messages on other models
    ├── responses.rs   Responses API → chat completions, for providers without /responses
    ├── router.rs      the router: deployments, retries, cooldowns, fallbacks
    ├── pricing.rs     rates and the cost formula (cache, reasoning, long-context tier, per unit)
    ├── catalog.rs     the price list parsed into rates and model facts
    ├── providers.rs   provider list (wire format, default base URL, stream usage support)
    ├── sse.rs         server-sent events split at event boundaries
    └── usage.rs       batched usage writer (logs, daily rollup, spend)
```

## Endpoints applications call

`/v1/chat/completions`, `/v1/completions`, `/v1/responses`, `/v1/embeddings`,
`/v1/images/generations`, `/v1/audio/speech`, `/v1/audio/transcriptions`,
`/v1/audio/translations`, `/v1/moderations`, `/v1/models`, Anthropic's
`/v1/messages` and `/v1/messages/count_tokens`, and `/v1/key/info`
(a key reads its own spend and limits).

## The router

Several models may share a public name: each row is a deployment, picked by
`weight` (evenly when none has one). Per request (`gateway/router.rs`):

- A failure is retried `num_retries` times (default 2) on another healthy
  deployment of the name, or the same one after a 0.5 s doubling backoff.
  Retried: unreachable, 408, 409, 429, 5xx; 401/403 only if the name has
  other deployments. Not retried: other 4xx.
- A deployment cools down for `cooldown_time` (5 s) on a 429/401/402/403/
  404/408, or after more than `allowed_fails` (3) failures in a minute. A
  name with one deployment never cools down. All cooling: 429 "No
  deployments available".
- When the name still fails, `fallbacks` are tried in order (each with its
  own retries); context-window and content-policy errors use their own
  lists first. `*` matches any name. A fallback the key may not call is
  skipped. Plain 4xx never fall back.
- Every attempt is logged (with `model_id`, the deployment) and billed to
  the name that answered. Responses carry `x-litellm-model-id`,
  `x-litellm-model-group`, `x-litellm-attempted-retries`,
  `x-litellm-attempted-fallbacks`.
- Settings live in `settings.router_settings` (JSON),
  `GET/PUT /api/router-settings`. Model access is by name, so ticking any
  deployment of a name allows all of it.

## Management API

The dashboard uses a session cookie. Scripts send `Authorization: Bearer`:
`MASTER_KEY` (or `LITELLM_MASTER_KEY`) acts as the first active admin; a
personal virtual key acts as its owner. Team keys are refused (403).

## Playground

`/api/playground/*` lets a signed-in user chat through a key they may change
(`keys::authorize`), named by id: the server looks up its hash and admits it
like `/v1` (`Gateway::admit_hash`), so limits, spend and logs apply, tagged
`playground`. The browser never holds a raw key. `models` lists what the key
may call; `cost` prices a reply's `usage` by the deployment in its
`x-litellm-model-id`, since a stream's cost is only known once it ends.

## Reports

`GET /api/usage/export?format=pdf|xlsx` takes the usage filters plus
`sections` (`services/report::SECTIONS`; all but `requests` by default),
`markup` (percent over provider cost), `show_cost`, `client`, `reference`,
`notes`. Language is the account's, then `Accept-Language`, then English;
currency and rate are the account's (the dashboard's rule). Without
`show_cost` a marked-up report shows only the amount, never the margin.
Filtered to a team, the team is the client unless one is given, and a
breakdown a filter already answers (teams, keys, people, models) is left
out. The PDF is one document: the cover and the contents are unnumbered
`section`s (no bands), chapters carry `anchor`s (also the PDF outline), and
the contents prints `{{pageof:id}}` and links to `#id`. Blocks that must
not split (charts, KPI rows, callouts) are one box. Geist is the default
family and Geist Mono is registered as `mono` (wordmark, numbers of
chapters).
Rendering is imprenta (git dependency, pinned by release tag; fonts in
`assets/fonts`), off the async threads. Previews: `REPORT_PREVIEW_DIR=dir
cargo test --lib report` writes a sample PDF and XLSX in both languages.

## Attribution

Spend is also reported by end user (customers) and by tag. End
user: `x-litellm-customer-id`/`x-litellm-end-user-id`, then `user`,
`litellm_metadata.user`, `metadata.user_id`, `safety_identifier` (256
chars). Tags: `x-litellm-tags`, `tags`, `metadata.tags`,
`litellm_metadata.tags` (20 of 64 chars). Tags and `litellm_metadata` are
taken out of the body before it reaches the provider. Rolled up in
`usage_daily_end_users` and `usage_daily_tags`; logs filter by both.

## Keys, teams, invitations

- A key belongs to a team (bills to it, shares its budget and models) or is
  personal (`team_id` null, `user_id` set: bills to its user, may call every
  model). Usage rows carry both; reports group personal spend under a null team.
- People (`people`) are who spend through a team's keys without a dashboard
  account; dashboard accounts stay in `users`. Created on their own
  (`/api/people`) and placed in at most one team; teamless people are for
  global admins, a team's people for its admins. A team key may be one
  person's (`api_keys.person_id`); the usage writer stamps it on
  `usage_daily.person_id`, so reassigning a key does not move its history.
  Reports break down `by_person` and group the series by `person`.
- A team's model access is explicit: `all_models`, or the
  ticked `team_models` (none ticked: no model). A key's own list narrows it.
- Keys can be blocked (reversible), regenerated (new secret, same row, spend
  and history) or revoked.
- Budgets reset per period (`budget_duration`: `30s`, `1h`, `1d`, `7d`,
  `1mo`), lazily: the first request after `budget_reset_at` zeroes the spend
  and sets the next reset. No timer runs.
- `rpm_limit`/`tpm_limit` on keys and teams: fixed one-minute windows in
  memory; requests count when admitted, tokens when they finish.
- `max_parallel_requests` on keys and teams: requests in flight
  at once. Admission takes a `Slot`, held by the request's `Caller` until
  the response (stream included) is done; over the limit is a 429.
- Invitations are Rustrak's: an admin creates one (48 h, token in the link,
  no email sent), the invitee sets a password at `/invite/{token}` and is
  signed in.

- OpenAI-dialect providers (OpenAI, Azure, Gemini, Groq, Mistral, DeepSeek,
  OpenRouter, Together, Ollama, any compatible server) get every endpoint with
  the body passed through, so provider-specific fields reach the provider.
- Anthropic models take chat completions (translated), `/v1/messages`
  (native, `anthropic-version`/`anthropic-beta` passed on) and `/v1/responses`
  (translated twice). Other endpoints answer 400.
- `/v1/responses` is passed through to OpenAI and Azure; for every other
  provider it is translated to chat completions and back, streams included
  (`gateway/responses.rs`). Stateless only: `previous_response_id` is refused.
- `/v1/messages` on a non-Anthropic model is translated through chat
  completions, which is what lets Claude Code run on any model.

## Pricing

Rates are micro-USD per million tokens (or per image, second, million
characters). `models.pricing_mode` says how a model is priced: `catalog`, by
its price-list entry (`catalog_key`, picked, or matched by name when not);
`custom`, by its own rates (`models.pricing`, JSON); or `free`, deliberately
$0 (tokens still count). A `catalog` model the list lacks reports
`pricing_source: none`, which the dashboard warns about; `free` does not. The catalog ships in the binary and an admin can sync it from
its URL (`pricing/model_prices.json` in this repo); the synced copy is kept in `settings`. The formula:
prompt tokens include cache reads and writes, which are taken out
and billed at their rates; reasoning tokens bill at their rate or the output
rate; past the long-context threshold every rate switches tier.

## The hot path

1. `Authorization: Bearer sk-…` (or `x-api-key`) → SHA-256 → in-memory key
   cache. The database is read only on a miss.
2. Blocked, expiry, budgets and per-minute limits are checked against
   **in-memory counters** that requests bump the moment they finish; the
   database catches up later. A budget whose period ended is reset first.
3. The model name resolves through an in-memory route table (provider keys
   already decrypted). Any change to keys, teams or models calls
   `Gateway::invalidate`, which drops both caches; spend counters survive it.
4. OpenAI-wire providers get the body forwarded with the model name swapped;
   Anthropic is translated both ways. Streams are forwarded event by event;
   `stream_options.include_usage` is forced on where the provider accepts it
   and the usage-only chunk is stripped again if the client did not ask for
   it; elsewhere a stream without usage is estimated from its text.
5. Usage goes to a channel; one task writes whatever queued up in a single
   transaction: `request_logs`, `usage_daily` (upsert), key and team spend.

Provider 401/403 become 502 (the client's key is fine, ours is not); other
provider errors pass through in OpenAI's shape. A client that disconnects
mid-stream is still billed for what was generated (status 499).

## Two databases

SQL is written once in the dialect both accept (`$n` placeholders, `ON
CONFLICT`, `RETURNING`, `CAST(… AS BIGINT)`). Migrations exist twice, in
`migrations/sqlite` and `migrations/postgres`, and must change together.
The schema starts as one migration (squashed before the first release);
from 0.1.0 on, changes are new migrations, never edits to published ones.
Timestamps are always bound from Rust, never defaulted by the database:
SQLite compares them as strings.

## Tests

```bash
cargo test                                            # unit (in-module) + integration
cargo test --no-default-features --features postgres  # same suites on PostgreSQL (Docker)
```

`tests/common` gives each test its own database (SQLite: copy of a migrated
template; PostgreSQL: a testcontainer), the real `app::configure`, and
`tests/common/upstream.rs`, a fake OpenAI + Anthropic provider on a real socket.
Write the test first.

## Conventions

- `AppError` everywhere; never `unwrap` on a request path.
- Raw virtual keys are shown once and never stored or logged.
- Provider API keys never leave the server (responses say `has_api_key`).
- Prompts and completions are never stored; logs keep what billing needs.

## Known ceilings (v1)

- Budgets are check-then-charge: concurrent requests can overshoot by what
  they cost. Reserve an estimate up front if hard caps matter.
- A failed usage batch is logged and dropped.
- Spend counters, rate-limit windows and deployment health are per process.
  Several replicas need a shared store (Redis).
- Usage still queued when a budget period resets lands in the new period.
- Routing is simple shuffle only (no latency- or usage-based
  strategies); no Bedrock/Vertex (their signing is not OpenAI's), no batch or service-tier
  prices.
- A whisper transcription without a usage block bills $0: the gateway does
  not decode audio to learn its length.
