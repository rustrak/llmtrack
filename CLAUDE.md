# llmtrack

A self-hosted LLM gateway, built the Rustrak way: one Rust
binary that applications call with an OpenAI SDK, routing each request to the
provider behind a model name and billing it to a team.

```
OpenAI SDK  ──▶  llmtrack (Rust/Actix-web)  ──▶  OpenAI, Anthropic, Groq, vLLM, …
(any app,         virtual key → team,            (provider keys stay here)
 sk- key)         model → provider,
                  spend → SQLite / PostgreSQL
```

**One process, one image.** The dashboard is a Vite SPA compiled into
`apps/server/static`; the Actix instance that proxies `/v1` also answers `/api`
and `/`, so the session cookie is first-party and there is no CORS. The
dashboard stays optional: `cargo build` works without Node.

## Layout

| Path | What |
|---|---|
| `apps/server` | Rust server: gateway, management API, dashboard host. The product. |
| `apps/dashboard` | `@llmtrack/dashboard`, the SPA the server serves (English and Spanish) |
| `packages/benchmarks` | Added latency and resources, against a mock provider and other gateways |

Each app has its own `CLAUDE.md`. Read it before working inside it.

## Commands

```bash
cd apps/server && cargo run                    # :4000, SQLite (needs SECRET_KEY, see .env.example)
pnpm dev                                       # dashboard on :3000, proxied to :4000

docker compose up -d postgres                  # PostgreSQL instead
cd apps/server && cargo run --no-default-features --features postgres

(cd apps/server && cargo test)                                       # SQLite suites
(cd apps/server && cargo test --no-default-features --features postgres)  # needs Docker
pnpm --filter @llmtrack/dashboard test         # dashboard unit + architecture tests
(cd packages/benchmarks && bash scripts/compare.sh)  # comparison, needs Docker
pnpm run ci                                    # everything CI runs
```

First run: `SECRET_KEY=$(openssl rand -hex 32)` and
`CREATE_SUPERUSER="admin@example.com:password"`.

## Conventions

- Rust through `rustfmt` and `clippy -D warnings`; TypeScript through Biome.
- Tests come first and with the change: server changes are written test-first
  against a real database and a fake provider.
- Money is integers end to end (nano-USD), floats only at the JSON edge.
- Prices follow the price list (`pricing/model_prices.json`, bundled, syncable from Settings → Pricing) or
  a model's own rates; cache, reasoning and long-context tokens bill apart.
- Every sentence in the dashboard is in `en.json` and `es.json`; a test fails
  when one is missing from either.
- Conventional commits, in English.
- Releases go through changesets (`server` and `dashboard` are one fixed
  version); the `llmtrack-release-commit` skill writes the changeset.
