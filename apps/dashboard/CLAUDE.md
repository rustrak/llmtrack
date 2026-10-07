# llmtrack dashboard

Vite + React 19 SPA, TanStack Router (file-based, loaders), Tailwind 4, the
shadcn/Base UI components and tokens of the Rustrak dashboard. Compiled to
`dist/`, copied into `apps/server/static` by `scripts/bundle-dashboard.sh`.

## Rules

- `routes/` → `features/<slice>/{api,model,ui}` → `shared/`. Layers import
  downward only and features never import each other; a test enforces it.
- The browser talks only to its own origin. `vite.config.ts` proxies
  `shared/config/api-prefixes.ts`, which must match `API_PREFIXES` in
  `apps/server/src/routes/dashboard.rs` (tested).
- One auth gate: `_authenticated` `beforeLoad`. The session has three states;
  `unavailable` shows an outage screen instead of bouncing to login.
- A screen's data comes from its loader; mutations end in `router.invalidate()`.
- Every API call returns a `Result` (`shared/api/http.ts`); server shapes are
  zod schemas in `shared/api/schemas.ts`.
- Form logic that can be pure lives in `features/*/model` with a test.
- shadcn files under `shared/ui/components/shadcn` are copied, not edited.
- i18n is Rustrak's, unchanged: `use-intl`, catalogues in
  `shared/i18n/messages/{en,es}.json`, locale from the account, then the
  browser, then English; the zone from the account, then UTC. Components call
  `useTranslations('<namespace>')`, route heads `translator()`. Numbers, money
  and dates go through `useFormatter` (`shared/ui/hooks/use-money.ts`); `new
  Intl.*` and `toLocale*` are banned outside tests. Two architecture tests
  keep every key present in both languages and every `t()` call resolvable,
  so one file must not bind two namespaces to the same name.
- Changing language or zone calls `intl.reload()`, so it applies at once.
- Settings: personal (profile, security, appearance) for everyone; instance
  (general, pricing) for admins. Anything instance-wide lives there, not in
  menus.

## Commands

```bash
pnpm dev            # :3000, proxies to LLMTRACK_SERVER_URL (default :4000)
pnpm test           # vitest: shared/lib, shared/api, features/*/model, architecture
pnpm check-types && pnpm lint && pnpm build
```
