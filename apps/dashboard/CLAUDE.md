# llmtrack dashboard

Vite + React 19 SPA, TanStack Router (file-based, loaders), Tailwind 4, and
Rustrak's design system `@rustrak/ui` (Base UI, Hugeicons, Inter + Geist Mono).
Compiled to `dist/`, copied into `apps/server/static` by
`scripts/bundle-dashboard.sh`.

## Design system

- `@rustrak/ui` comes from GitHub, pinned to a commit in `package.json`
  (`github:rustrak/rustrak#<sha>&path:packages/ui`). Its `dist/` is not built
  there, so `vite.config.ts`, `vitest.config.ts` and `tsconfig.json` alias the
  package to its `src/index.ts`. To update it, change the sha and
  `pnpm install`.
- `src/styles.css` imports the closed system (`@rustrak/ui/styles.css`):
  Tailwind's palette, type scale, radii and shadows are reset, so only the
  tokens exist (`text-fg-subtle`, `bg-surface`, `text-body`, `rounded-xl`…).
  `text-sm` or `bg-red-500` compile to nothing. Layout utilities stay.
- Icons come from the design system's catalogue (`NewIcon`, `DeleteIcon`…);
  llmtrack's own glyphs are in `shared/ui/icons.ts`, through `fromHugeicons`.
- The shell is the design system's: `AppShell`, `Sidebar` (two modes, the app
  and settings, picked by the URL), `Topbar` with `Breadcrumbs`. A detail page
  names itself in the trail with `usePageTrail`.
- Settings pages follow Rustrak's: `SettingsHeader`, then `SettingSection`s of
  `SettingRow`s (`shared/ui/components/setting-row.tsx`).

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
- i18n is Rustrak's, unchanged: `use-intl`, catalogues in
  `shared/i18n/messages/{en,es}.json`, locale from the account, then the
  browser, then English; the zone from the account, then UTC. Components call
  `useTranslations('<namespace>')`, route heads `translator()`. Numbers, money
  and dates go through `useFormatter` (`shared/ui/hooks/use-money.ts`); `new
  Intl.*` and `toLocale*` are banned outside tests. Two architecture tests
  keep every key present in both languages and every `t()` call resolvable,
  so one file must not bind two namespaces to the same name.
- Changing language or zone calls `intl.reload()`, so it applies at once.
- Settings: personal (profile, preferences) for everyone; instance (MCP for
  everyone, general and pricing for admins). Anything instance-wide lives
  there, not in menus.

## Commands

```bash
pnpm dev            # :3000, proxies to LLMTRACK_SERVER_URL (default :4000)
pnpm test           # vitest: shared/lib, shared/api, features/*/model, architecture
pnpm check-types && pnpm lint && pnpm build
```
