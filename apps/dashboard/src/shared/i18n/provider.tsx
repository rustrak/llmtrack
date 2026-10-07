import { useEffect, useSyncExternalStore } from 'react';
import { IntlProvider } from 'use-intl';
import { intl } from './intl';

/**
 * The messages, for the whole application.
 *
 * **One provider, not two.** Under Next there were two, and the split was
 * worth it: `NextIntlClientProvider` serialises whatever it is given into the
 * RSC payload of every page under it, so handing all 30 namespaces to
 * `/login` made the one page a visitor sees before they are anyone a
 * 113KB document carrying the copy for source-map cleanup. There is no RSC
 * payload here. The catalogue is one lazily-imported chunk that the browser
 * fetches once and caches, so a second provider would split a cost that is no
 * longer paid per page.
 *
 * It re-renders when the reader changes their language, which is the other
 * thing Next did for us: `router.refresh()` re-ran the request config against
 * the new row. `intl.reload()` is that same act, and this subscription is what
 * turns it into a repaint.
 */
export function Messages({ children }: { children: React.ReactNode }) {
  const snapshot = useSyncExternalStore(
    intl.subscribe,
    intl.snapshot,
    intl.snapshot,
  );

  // `<html lang>` was rendered per request by the server. Nothing but this
  // sets it on a static shell, and it is what a screen reader picks a voice
  // from — the one piece of the document Tailwind cannot express.
  useEffect(() => {
    document.documentElement.lang = snapshot.locale;
  }, [snapshot.locale]);

  return (
    <IntlProvider
      locale={snapshot.locale}
      messages={snapshot.messages}
      timeZone={snapshot.timeZone}
      formats={snapshot.formats}
      now={snapshot.now}
    >
      {children}
    </IntlProvider>
  );
}
