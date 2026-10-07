import { ChevronRight, type LucideIcon, X } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslations } from 'use-intl';
import { cn } from '@/shared/lib/utils';
import { Button } from './shadcn/button';
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetTitle,
} from './shadcn/sheet';

/**
 * Where records are created and edited: a panel from the right, after
 * Twenty's. A header with the record's icon and title, a body that scrolls,
 * and `PanelFooter` pinned at the bottom. A click outside does not close it,
 * so a half-filled form or a key shown once is not lost by accident.
 */
export function SidePanel({
  open,
  onOpenChange,
  icon: Icon,
  title,
  description,
  children,
  onClosed,
  size = 'md',
}: {
  open: boolean;
  /** `lg` for forms with tables or side-by-side fields. */
  size?: 'md' | 'lg';
  onOpenChange: (open: boolean) => void;
  /** After the closing animation: where to reset state, out of sight. */
  onClosed?: () => void;
  icon: LucideIcon;
  title: ReactNode;
  description?: ReactNode;
  /** `PanelBody` and `PanelFooter`, usually inside a `<form>`. */
  children: ReactNode;
}) {
  const t = useTranslations('common');
  return (
    <Sheet
      open={open}
      onOpenChange={onOpenChange}
      onOpenChangeComplete={(isOpen) => !isOpen && onClosed?.()}
      disablePointerDismissal
    >
      <SheetContent
        showCloseButton={false}
        data-size={size}
        // A slide from the edge, not shadcn's short nudge and fade.
        className="w-full gap-0 duration-300 ease-[cubic-bezier(0.32,0.72,0,1)] data-[side=right]:sm:max-w-[460px] data-[size=lg]:data-[side=right]:sm:max-w-[600px] data-[side=right]:data-starting-style:translate-x-full data-[side=right]:data-ending-style:translate-x-full data-starting-style:opacity-100 data-ending-style:opacity-100"
      >
        <header className="flex shrink-0 items-start gap-3 border-b px-5 py-4">
          <span className="flex size-9 shrink-0 items-center justify-center rounded-lg border bg-muted/50 text-muted-foreground">
            <Icon className="size-4" />
          </span>
          <div className="min-w-0 flex-1 space-y-0.5 pt-0.5">
            <SheetTitle className="text-base leading-tight font-semibold">
              {title}
            </SheetTitle>
            {description && (
              <SheetDescription className="text-xs leading-relaxed">
                {description}
              </SheetDescription>
            )}
          </div>
          <SheetClose
            render={
              <Button
                variant="ghost"
                size="icon-sm"
                className="-mt-0.5 -mr-1.5"
              />
            }
          >
            <X />
            <span className="sr-only">{t('close')}</span>
          </SheetClose>
        </header>
        {children}
      </SheetContent>
    </Sheet>
  );
}

/** The panel's scrolling middle. */
export function PanelBody({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-5 py-5',
        className,
      )}
    >
      {children}
    </div>
  );
}

/** Actions, right-aligned and always in view. */
export function PanelFooter({ children }: { children: ReactNode }) {
  return (
    <div className="flex shrink-0 items-center justify-end gap-2 border-t bg-muted/30 px-5 py-3">
      {children}
    </div>
  );
}

/**
 * A group of optional settings, folded: its title, what it is set to now,
 * and the fields once opened.
 */
export function PanelSection({
  title,
  summary,
  children,
  forceOpen,
  defaultOpen,
}: {
  title: string;
  /** The current value in a few words, shown while folded. */
  summary: string;
  children: ReactNode;
  /** Opens it, e.g. when one of its fields has an error. */
  forceOpen?: boolean;
  /** Open when the panel opens; the user can still fold it. */
  defaultOpen?: boolean;
}) {
  return (
    <details
      ref={(el) => {
        // Uncontrolled, so set once on mount rather than through `open`.
        if (el && defaultOpen && !el.dataset.init) el.open = true;
        if (el) el.dataset.init = '1';
      }}
      open={forceOpen || undefined}
      className="group/section rounded-lg border bg-card/40 open:bg-transparent"
    >
      <summary className="flex cursor-pointer list-none items-center gap-2 rounded-lg px-3 py-2.5 text-sm transition-colors select-none hover:bg-muted/50 [&::-webkit-details-marker]:hidden">
        <ChevronRight className="size-4 text-muted-foreground transition-transform group-open/section:rotate-90" />
        <span className="font-medium">{title}</span>
        <span className="ml-auto truncate text-xs text-muted-foreground">
          {summary}
        </span>
      </summary>
      <div className="border-t px-3 pt-3 pb-4">{children}</div>
    </details>
  );
}

/** `⌘↵` or `Ctrl↵` submits, as in Twenty and Linear. */
export function submitOnModEnter(event: React.KeyboardEvent<HTMLFormElement>) {
  if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
    event.preventDefault();
    event.currentTarget.requestSubmit();
  }
}

/** The shortcut hint inside a submit button. */
export function ModEnterHint() {
  const mac =
    typeof navigator !== 'undefined' &&
    /Mac|iPhone|iPad/.test(navigator.platform);
  return (
    <kbd className="ml-1 rounded border border-current/20 px-1 font-sans text-[10px] opacity-70">
      {mac ? '⌘↵' : 'Ctrl↵'}
    </kbd>
  );
}
