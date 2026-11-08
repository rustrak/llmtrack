import { AppearanceIcon, type IconComponent, MemberIcon } from '@rustrak/ui';
import { useTranslations } from 'use-intl';
import type { User } from '@/shared/api/schemas';
import {
  GeneralIcon,
  KeysIcon,
  LabelsIcon,
  McpIcon,
  ModelsIcon,
  PeopleIcon,
  PlaygroundIcon,
  PricingIcon,
  RequestLogsIcon,
  TeamsIcon,
  UsageIcon,
  UsersIcon,
} from '@/shared/ui/icons';

export type NavItem = { to: string; label: string; icon: IconComponent };
export type NavGroup = { label: string; items: NavItem[] };

/** The sidebar's sections, which the top bar also names the page by. */
export function useNav(user: User): NavGroup[] {
  const t = useTranslations('nav');
  return [
    {
      label: t('gateway'),
      items: [
        { to: '/keys', label: t('keys'), icon: KeysIcon },
        { to: '/models', label: t('models'), icon: ModelsIcon },
        { to: '/playground', label: t('playground'), icon: PlaygroundIcon },
        { to: '/teams', label: t('teams'), icon: TeamsIcon },
        { to: '/people', label: t('people'), icon: PeopleIcon },
        { to: '/labels', label: t('labels'), icon: LabelsIcon },
      ],
    },
    {
      label: t('insights'),
      items: [
        { to: '/usage', label: t('usage'), icon: UsageIcon },
        { to: '/logs', label: t('logs'), icon: RequestLogsIcon },
      ],
    },
    ...(user.role === 'admin'
      ? [
          {
            label: t('manage'),
            items: [{ to: '/users', label: t('users'), icon: UsersIcon }],
          },
        ]
      : []),
  ];
}

/**
 * Every settings page, in the sidebar's settings mode: the reader's own
 * account, then the instance's, the second half for admins only.
 */
export function useSettingsNav(user: User): NavGroup[] {
  const tSettings = useTranslations('settings');
  const isAdmin = user.role === 'admin';
  return [
    {
      label: tSettings('nav.personal'),
      items: [
        {
          to: '/settings/profile',
          label: tSettings('nav.profile'),
          icon: MemberIcon,
        },
        {
          to: '/settings/preferences',
          label: tSettings('nav.preferences'),
          icon: AppearanceIcon,
        },
      ],
    },
    {
      label: tSettings('nav.instance'),
      items: [
        { to: '/settings/mcp', label: tSettings('nav.mcp'), icon: McpIcon },
        ...(isAdmin
          ? [
              {
                to: '/settings/general',
                label: tSettings('nav.general'),
                icon: GeneralIcon,
              },
              {
                to: '/settings/pricing',
                label: tSettings('nav.pricing'),
                icon: PricingIcon,
              },
            ]
          : []),
      ],
    },
  ];
}

export const isUnder = (pathname: string, to: string) =>
  pathname === to || pathname.startsWith(`${to}/`);

/** Settings pages put the sidebar in its settings mode. */
export const isSettingsPath = (pathname: string) =>
  isUnder(pathname, '/settings');
