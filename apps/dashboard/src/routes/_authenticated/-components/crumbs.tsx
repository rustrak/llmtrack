import { Link, useRouterState } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import { useTranslations } from 'use-intl';
import type { User } from '@/shared/api/schemas';
import { isUnder, useNav } from './nav';

/** Where the page sits: its sidebar section, then the page itself. */
export function Crumbs({ user }: { user: User }) {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const t = useTranslations('nav');
  const groups = useNav(user);
  for (const group of groups) {
    const item = group.items.find((x) => isUnder(pathname, x.to));
    if (!item) continue;
    const Icon = item.icon;
    return (
      <nav
        aria-label={t('breadcrumb')}
        className="flex min-w-0 items-center gap-1.5 text-sm"
      >
        <span className="hidden text-muted-foreground sm:inline">
          {group.label}
        </span>
        <ChevronRight className="hidden size-3.5 text-muted-foreground/60 sm:block" />
        <Link
          to={item.to}
          className="flex items-center gap-1.5 truncate rounded-md px-1 py-0.5 font-medium transition-colors hover:bg-muted"
        >
          <Icon className="size-4 text-muted-foreground" />
          {item.label}
        </Link>
      </nav>
    );
  }
  return null;
}
