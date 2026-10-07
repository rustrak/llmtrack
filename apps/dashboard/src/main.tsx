import '@fontsource-variable/geist';
import '@fontsource-variable/geist-mono';
import './styles.css';

import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { session } from '@/shared/api/session';
import { intl } from '@/shared/i18n/intl';
import { Messages } from '@/shared/i18n/provider';
import { ThemeProvider } from '@/shared/ui/components/theme-provider';
import { Toaster } from '@/shared/ui/components/toaster';
import { createAppRouter } from './router';

const router = createAppRouter();

/**
 * The session and the catalogue are settled before the first render: the
 * router's guard never paints a page it is about to redirect away from, and
 * no screen flashes in English before the reader's language arrives.
 */
async function bootstrap() {
  await Promise.all([session.ensure(), intl.ensure()]).catch(() => undefined);

  const container = document.getElementById('root');
  if (!container) throw new Error('index.html is missing #root');

  createRoot(container).render(
    <StrictMode>
      <ThemeProvider
        attribute="class"
        defaultTheme="dark"
        enableSystem
        disableTransitionOnChange
      >
        <Messages>
          <RouterProvider router={router} />
          <Toaster />
        </Messages>
      </ThemeProvider>
    </StrictMode>,
  );
}

void bootstrap();
