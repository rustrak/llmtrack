import { SegmentedControl, SegmentedItem } from '@rustrak/ui';
import { useTheme } from 'next-themes';
import { useTranslations } from 'use-intl';
import { useIsHydrated } from '@/shared/ui/hooks/use-is-hydrated';

/**
 * Light, dark, or whatever the system says, as one segmented control.
 *
 * The theme is not knowable before hydration: next-themes reads localStorage
 * and the system preference, which only exist in the browser. Until then the
 * control renders with nothing chosen rather than guessing, so it never flips
 * under the reader's eye.
 */
export function ThemeSelector() {
  const { theme, setTheme } = useTheme();
  const hydrated = useIsHydrated();
  const t = useTranslations('theme');

  return (
    <SegmentedControl
      aria-label={t('label')}
      value={hydrated ? theme : undefined}
      onValueChange={setTheme}
      disabled={!hydrated}
    >
      <SegmentedItem value="light">{t('light')}</SegmentedItem>
      <SegmentedItem value="dark">{t('dark')}</SegmentedItem>
      <SegmentedItem value="system">{t('system')}</SegmentedItem>
    </SegmentedControl>
  );
}
