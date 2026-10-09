import {
  BarChart3,
  Boxes,
  Contact,
  KeyRound,
  MessagesSquare,
  ScrollText,
  Settings,
  Tag,
  UserCog,
  Users,
} from 'lucide-react';
import { useTranslations } from 'use-intl';
import type { User } from '@/shared/api/schemas';

export type NavItem = { to: string; label: string; icon: typeof KeyRound };
export type NavGroup = { label: string; items: NavItem[] };

/** The sidebar's sections, which the top bar also names the page by. */
export function useNav(user: User): NavGroup[] {
  const t = useTranslations('nav');
  return [
    {
      label: t('gateway'),
      items: [
        { to: '/keys', label: t('keys'), icon: KeyRound },
        { to: '/models', label: t('models'), icon: Boxes },
        { to: '/playground', label: t('playground'), icon: MessagesSquare },
        { to: '/teams', label: t('teams'), icon: Users },
        { to: '/people', label: t('people'), icon: Contact },
        { to: '/labels', label: t('labels'), icon: Tag },
      ],
    },
    {
      label: t('insights'),
      items: [
        { to: '/usage', label: t('usage'), icon: BarChart3 },
        { to: '/logs', label: t('logs'), icon: ScrollText },
      ],
    },
    {
      label: t('manage'),
      items: [
        ...(user.role === 'admin'
          ? [{ to: '/users', label: t('users'), icon: UserCog }]
          : []),
        { to: '/settings', label: t('settings'), icon: Settings },
      ],
    },
  ];
}

export const isUnder = (pathname: string, to: string) =>
  pathname === to || pathname.startsWith(`${to}/`);
