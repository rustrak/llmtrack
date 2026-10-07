import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

/**
 * Node, no DOM: the architecture rules plus the portable core (`shared/lib`,
 * `shared/api`, `shared/i18n`, each feature's `model`). Nothing here renders.
 *
 * One shared module registry (`isolate: false`): archunit builds the
 * TypeScript program once and every rule file reads the cached graph.
 * `globals` is what archunit's `toPassAsync` matcher needs.
 */
export default defineConfig({
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  test: {
    isolate: false,
    fileParallelism: false,
    environment: 'node',
    globals: true,
    include: [
      'src/__tests__/**/*.test.ts',
      'src/{features/*/model,shared/{lib,i18n,api}}/**/*.test.ts',
    ],
    exclude: ['**/node_modules/**', '**/dist/**'],
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
