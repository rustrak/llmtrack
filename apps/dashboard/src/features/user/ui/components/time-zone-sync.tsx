import { useEffect, useRef } from 'react';
import { intl } from '@/shared/i18n/intl';
import { updateProfile } from '../../api/mutations';

/**
 * Renders nothing. The first time someone arrives with no zone on their
 * account, adopts the browser's, so dates read the same on every device.
 */
export function TimeZoneSync({ hasTimeZone }: { hasTimeZone: boolean }) {
  const attempted = useRef(false);
  useEffect(() => {
    if (hasTimeZone || attempted.current) return;
    attempted.current = true;
    let zone: string;
    try {
      zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
    } catch {
      return;
    }
    if (!zone) return;
    void updateProfile({ timezone: zone }).then((result) => {
      if (result.success) void intl.reload();
    });
  }, [hasTimeZone]);
  return null;
}
