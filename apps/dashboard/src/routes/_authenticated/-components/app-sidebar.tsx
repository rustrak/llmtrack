import { Link, useRouter, useRouterState } from '@tanstack/react-router';
import { ArrowUpRight, ChevronsUpDown, LogOut, Settings } from 'lucide-react';
import { useTranslations } from 'use-intl';
import { logout } from '@/features/user/api/mutations';
import type { User } from '@/shared/api/schemas';
import { intl } from '@/shared/i18n/intl';
import { LlmtrackWordmark } from '@/shared/ui/components/llmtrack-wordmark';
import { RustrakWordmark } from '@/shared/ui/components/rustrak-wordmark';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/shared/ui/components/shadcn/dropdown-menu';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  useSidebar,
} from '@/shared/ui/components/shadcn/sidebar';
import { isUnder, type NavGroup, useNav } from './nav';

const RUSTRAK_URL = 'https://rustrak.github.io/rustrak/';

function NavSection({ label, items }: NavGroup) {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  return (
    <SidebarGroup>
      <SidebarGroupLabel>{label}</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          {items.map(({ to, label, icon: Icon }) => (
            <SidebarMenuItem key={to}>
              <SidebarMenuButton
                isActive={isUnder(pathname, to)}
                tooltip={label}
                render={<Link to={to} />}
              >
                <Icon />
                <span>{label}</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

export function AppSidebar({ user }: { user: User }) {
  const t = useTranslations('nav');
  const tRoles = useTranslations('roles');
  const router = useRouter();
  const { isMobile } = useSidebar();
  const nav = useNav(user);

  return (
    <Sidebar variant="inset" collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              size="lg"
              tooltip="llmtrack"
              className="h-auto py-2 transition-[background-color,translate] active:translate-y-px motion-reduce:transition-none"
              render={<Link to="/keys" />}
            >
              {/* Compact: the l of llm and the t of track, in a square. */}
              <span className="hidden size-8 shrink-0 items-center justify-center rounded-lg border border-sidebar-border bg-sidebar-accent font-mono text-sm font-bold group-data-[collapsible=icon]:flex">
                l<span className="text-primary">t</span>
              </span>
              <span className="flex flex-col gap-1.5 group-data-[collapsible=icon]:hidden">
                <LlmtrackWordmark className="text-xl leading-none" />
                <span className="text-xs leading-none text-muted-foreground">
                  {t('tagline')}
                </span>
              </span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        {nav.map((group) => (
          <NavSection key={group.label} {...group} />
        ))}
      </SidebarContent>
      <SidebarFooter>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <SidebarMenuButton
                size="lg"
                className="w-full data-popup-open:bg-sidebar-accent"
              >
                <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/20 font-mono text-xs font-bold text-primary uppercase">
                  {(user.name || user.email).slice(0, 2)}
                </div>
                <div className="grid flex-1 text-left leading-tight">
                  <span className="truncate text-sm font-medium">
                    {user.name || user.email}
                  </span>
                  <span className="truncate text-xs text-muted-foreground">
                    {tRoles(user.role)}
                  </span>
                </div>
                <ChevronsUpDown className="ml-auto size-4" />
              </SidebarMenuButton>
            }
          />
          <DropdownMenuContent
            side={isMobile ? 'bottom' : 'right'}
            align="end"
            sideOffset={8}
            className="w-56"
          >
            <DropdownMenuItem render={<Link to="/settings/profile" />}>
              <Settings />
              {t('profile')}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              onClick={async () => {
                await logout();
                await intl.reload();
                await router.navigate({ to: '/login' });
              }}
            >
              <LogOut />
              {t('logout')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <a
          href={RUSTRAK_URL}
          target="_blank"
          rel="noreferrer"
          className="group/credit mt-1 flex items-center gap-1.5 border-t border-sidebar-border px-2 pt-3 pb-1 text-xs text-muted-foreground transition-colors hover:text-foreground group-data-[collapsible=icon]:hidden"
        >
          {t('builtBy')}
          <RustrakWordmark className="h-3.5 w-auto" />
          <ArrowUpRight className="ml-auto size-3.5 opacity-0 transition-opacity group-hover/credit:opacity-100" />
        </a>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
