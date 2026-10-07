import {
  createFileRoute,
  Link,
  Outlet,
  useRouterState,
} from '@tanstack/react-router';
import {
  CircleDollarSign,
  Palette,
  SlidersHorizontal,
  User,
} from 'lucide-react';
import { useTranslations } from 'use-intl';
import { translator } from '@/shared/i18n/intl';
import { cn } from '@/shared/lib/utils';
import { Page } from '@/shared/ui/components/page';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';

export const Route = createFileRoute('/_authenticated/settings')({
  head: () => ({ meta: [{ title: translator('settings')('meta.title') }] }),
  component: SettingsLayout,
});

function SettingsLayout() {
  const t = useTranslations('settings');
  const isAdmin = useCurrentUser().role === 'admin';
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const sections = [
    {
      label: t('nav.personal'),
      items: [
        { to: '/settings/profile', label: t('nav.profile'), icon: User },
        {
          to: '/settings/appearance',
          label: t('nav.appearance'),
          icon: Palette,
        },
      ],
    },
    ...(isAdmin
      ? [
          {
            label: t('nav.instance'),
            items: [
              {
                to: '/settings/general',
                label: t('nav.general'),
                icon: SlidersHorizontal,
              },
              {
                to: '/settings/pricing',
                label: t('nav.pricing'),
                icon: CircleDollarSign,
              },
            ],
          },
        ]
      : []),
  ];

  return (
    <Page title={t('nav.title')} description={t('subtitle')}>
      <div className="grid gap-8 md:grid-cols-[12rem_minmax(0,1fr)]">
        {/* Styled like the app sidebar, so the two read as one system. */}
        <nav className="flex flex-col gap-5 md:sticky md:top-0 md:self-start">
          {sections.map((section) => (
            <div key={section.label} className="flex flex-col gap-0.5">
              <span className="px-2 pb-1 text-xs font-medium text-muted-foreground">
                {section.label}
              </span>
              {section.items.map(({ to, label, icon: Icon }) => (
                <Link
                  key={to}
                  to={to}
                  aria-current={pathname === to ? 'page' : undefined}
                  className={cn(
                    'flex h-8 items-center gap-2 rounded-md px-2 text-sm transition-colors',
                    pathname === to
                      ? 'bg-muted font-medium text-foreground'
                      : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground',
                  )}
                >
                  <Icon className="size-4" />
                  {label}
                </Link>
              ))}
            </div>
          ))}
        </nav>
        <div className="min-w-0">
          <Outlet />
        </div>
      </div>
    </Page>
  );
}
