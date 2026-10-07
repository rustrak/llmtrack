import { fileURLToPath } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import viteReact from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';
import { API_PREFIXES } from './src/shared/config/api-prefixes';

/**
 * A pure SPA: compiled to static files the Rust server hands out. The browser
 * always talks to its own origin; in development Vite owns that origin and
 * proxies the API prefixes to the server, which keeps the session cookie
 * first-party and CORS out of the picture.
 */
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const server = env.LLMTRACK_SERVER_URL || 'http://127.0.0.1:4000';

  const proxy = Object.fromEntries(
    API_PREFIXES.map((prefix) => [
      prefix,
      {
        target: server,
        changeOrigin: true,
        // The server marks the cookie `Secure` behind SSL_PROXY; over plain
        // http the browser would drop it, so the dev proxy strips it.
        cookieDomainRewrite: '',
        configure: (proxyServer: {
          on: (
            event: string,
            listener: (proxyRes: {
              headers: Record<string, string | string[] | undefined>;
            }) => void,
          ) => void;
        }) => {
          proxyServer.on('proxyRes', (proxyRes) => {
            const cookies = proxyRes.headers['set-cookie'];
            if (Array.isArray(cookies)) {
              proxyRes.headers['set-cookie'] = cookies.map((cookie) =>
                cookie.replace(/;\s*Secure/gi, ''),
              );
            }
          });
        },
      },
    ]),
  );

  return {
    base: '/',
    resolve: {
      alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
    },
    plugins: [
      tailwindcss(),
      tanstackRouter({ target: 'react', autoCodeSplitting: true }),
      viteReact(),
    ],
    server: { port: 3000, proxy },
    preview: { port: 3000, proxy },
    build: { outDir: 'dist', emptyOutDir: true, assetsDir: 'assets' },
  };
});
