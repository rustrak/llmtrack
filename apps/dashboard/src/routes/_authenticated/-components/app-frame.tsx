import {
  BackIcon,
  Breadcrumbs,
  type Crumb,
  SettingsIcon,
  Sidebar,
  SidebarBrand,
  SidebarGroup,
  SidebarItem,
  SidebarScope,
  SidebarUser,
  SignOutIcon,
} from '@rustrak/ui';
import { Link, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import { logout } from '@/features/user/api/mutations';
import type { User } from '@/shared/api/schemas';
import { intl } from '@/shared/i18n/intl';
import { LlmtrackWordmark } from '@/shared/ui/components/llmtrack-wordmark';
import { usePageTrailCrumbs } from '@/shared/ui/hooks/use-page-trail';
import { usePathname } from '@/shared/ui/hooks/use-pathname';
import {
  isSettingsPath,
  isUnder,
  type NavGroup,
  useNav,
  useSettingsNav,
} from './nav';

/**
 * The frame every signed-in screen sits in: the sidebar, and the trail at the
 * top of the sheet.
 *
 * The sidebar has two modes and the URL picks between them, one level deeper
 * than the other: the dashboard (its routes) and settings (every settings
 * page, the reader's and the instance's). Moving between them is a
 * `SidebarScope` change, so it slides, as Rustrak's does.
 */

function Groups({ groups }: { groups: NavGroup[] }) {
  const pathname = usePathname();
  return groups.map((group) => (
    <SidebarGroup key={group.label} label={group.label}>
      {group.items.map((item) => (
        <SidebarItem
          key={item.to}
          icon={item.icon}
          label={item.label}
          active={isUnder(pathname, item.to)}
          render={<Link to={item.to} />}
        />
      ))}
    </SidebarGroup>
  ));
}

export function AppSidebar({ user }: { user: User }) {
  const t = useTranslations('nav');
  const tSettings = useTranslations('settings');
  const tRoles = useTranslations('roles');
  const router = useRouter();
  const pathname = usePathname();
  const nav = useNav(user);
  const settingsNav = useSettingsNav(user);
  const settingsMode = isSettingsPath(pathname);
  const [signingOut, setSigningOut] = useState(false);

  const signOut = async () => {
    setSigningOut(true);
    try {
      await logout();
      await intl.reload();
      await router.navigate({ to: '/login' });
    } finally {
      setSigningOut(false);
    }
  };

  return (
    <Sidebar
      header={
        <SidebarBrand
          switcher={
            <Link
              to="/keys"
              aria-label="llmtrack"
              className="flex h-7 w-fit items-center rounded-xs px-2 outline-none focus-visible:outline-2 focus-visible:outline-ring"
            >
              <LlmtrackWordmark className="text-[1.0625rem] leading-none" />
            </Link>
          }
        />
      }
      footer={
        <>
          {/* Inside settings the way out is the back row at the top, and a
              settings link would point at the page you are on. */}
          {settingsMode ? null : (
            <SidebarItem
              icon={SettingsIcon}
              label={t('settings')}
              render={<Link to="/settings/profile" />}
            />
          )}
          <SidebarUser
            name={user.name || user.email}
            detail={tRoles(user.role)}
            email={user.email}
            actions={[
              {
                id: 'settings',
                label: t('profile'),
                icon: SettingsIcon,
                onSelect: () => router.navigate({ to: '/settings/profile' }),
              },
              {
                id: 'sign-out',
                label: t('logout'),
                icon: SignOutIcon,
                tone: 'danger',
                separated: true,
                disabled: signingOut,
                onSelect: () => void signOut(),
              },
            ]}
          />
        </>
      }
    >
      <SidebarScope
        scope={settingsMode ? 'settings' : 'app'}
        depth={settingsMode ? 1 : 0}
      >
        {settingsMode ? (
          <>
            <SidebarItem
              icon={BackIcon}
              label={tSettings('nav.back')}
              render={<Link to="/keys" />}
            />
            <Groups groups={settingsNav} />
          </>
        ) : (
          // The first group sits right under the mark: its label keeps the
          // gap the design gives a group after items, which is none here.
          <div className="contents [&>*:first-child>*:first-child]:mt-0">
            <Groups groups={nav} />
          </div>
        )}
      </SidebarScope>
    </Sidebar>
  );
}

/**
 * Where you are, at the top of the sheet: the section, then whatever the page
 * says it is (a key's name, a team's). Below the section's own page, the
 * section crumb is the way back to it.
 */
export function AppBreadcrumbs({ user }: { user: User }) {
  const t = useTranslations('nav');
  const tSettings = useTranslations('settings');
  const pathname = usePathname();
  const trail = usePageTrailCrumbs();
  const nav = useNav(user);
  const settingsNav = useSettingsNav(user);

  const trailCrumbs: Crumb[] = trail.map((crumb) => ({
    label: crumb.label,
    mono: crumb.mono,
    render: crumb.href ? <Link to={crumb.href} /> : undefined,
  }));

  let items: Crumb[] = [];
  if (isSettingsPath(pathname)) {
    const page = settingsNav
      .flatMap((group) => group.items)
      .find((item) => isUnder(pathname, item.to));
    items = [
      {
        label: tSettings('nav.title'),
        render: <Link to="/settings" />,
      },
      ...(page ? [{ label: page.label }] : []),
    ];
  } else {
    const item = nav
      .flatMap((group) => group.items)
      .find((entry) => isUnder(pathname, entry.to));
    if (item) {
      const below = pathname.length > item.to.length + 1;
      items =
        below && trailCrumbs.length > 0
          ? [
              { label: item.label, render: <Link to={item.to} /> },
              ...trailCrumbs,
            ]
          : [{ label: item.label }];
    }
  }

  return <Breadcrumbs items={items} label={t('breadcrumb')} />;
}
