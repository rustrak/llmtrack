import { Dialog as BaseDialog } from '@base-ui/react/dialog';
import {
  Button,
  ChevronRightIcon,
  CloseIcon,
  type IconComponent,
  Kbd,
} from '@rustrak/ui';
import type { ReactNode } from 'react';
import { useTranslations } from 'use-intl';
import { cn } from '@/shared/lib/utils';

/**
 * Where records are created and edited: a sheet from the right, on the design
 * system's surfaces. A header with the record's icon and title, a body that
 * scrolls, and `PanelFooter` pinned at the bottom. A click outside does not
 * close it, so a half-filled form or a key shown once is not lost by accident.
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
  icon: IconComponent;
  title: ReactNode;
  description?: ReactNode;
  /** `PanelBody` and `PanelFooter`, usually inside a `<form>`. */
  children: ReactNode;
}) {
  const t = useTranslations('common');
  return (
    <BaseDialog.Root
      open={open}
      onOpenChange={onOpenChange}
      onOpenChangeComplete={(isOpen) => !isOpen && onClosed?.()}
      disablePointerDismissal
    >
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className="fixed inset-0 z-50 bg-scrim transition-[opacity] duration-moderate ease-standard data-ending-style:opacity-0 data-starting-style:opacity-0" />
        <BaseDialog.Popup
          className={cn(
            'fixed inset-y-0 right-0 z-50 flex w-full flex-col outline-none',
            'border-border border-l bg-surface shadow-dialog',
            'sm:inset-y-2 sm:right-2 sm:rounded-2xl sm:border',
            size === 'lg' ? 'sm:max-w-[600px]' : 'sm:max-w-[460px]',
            'transition-[translate] duration-slow ease-entrance',
            'data-starting-style:translate-x-[105%] data-ending-style:translate-x-[105%] data-ending-style:ease-exit',
          )}
        >
          <header className="flex shrink-0 items-start gap-3 border-border-subtle border-b px-5.5 py-4">
            <span className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-border-subtle bg-surface-raised text-fg-subtle">
              <Icon size="lg" />
            </span>
            <div className="min-w-0 flex-1 space-y-0.5 pt-0.5">
              <BaseDialog.Title className="text-section text-fg">
                {title}
              </BaseDialog.Title>
              {description && (
                <BaseDialog.Description className="text-meta text-fg-subtle">
                  {description}
                </BaseDialog.Description>
              )}
            </div>
            <BaseDialog.Close
              render={
                <Button
                  variant="ghost"
                  size="sm"
                  icon={CloseIcon}
                  aria-label={t('close')}
                  className="-mt-0.5 -mr-1.5"
                />
              }
            />
          </header>
          {children}
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
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
        'flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-5.5 py-5',
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
    <div className="flex shrink-0 items-center justify-end gap-2.25 border-border-subtle border-t bg-surface-raised px-5.5 py-4 sm:rounded-b-2xl">
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
      className="group/section rounded-xl border border-border-subtle bg-surface-raised open:bg-transparent"
    >
      <summary className="flex h-control-lg cursor-pointer list-none items-center gap-2 rounded-xl px-3 text-control select-none hover:bg-surface-hover [&::-webkit-details-marker]:hidden">
        <ChevronRightIcon
          size="sm"
          className="text-fg-ghost transition-[rotate] duration-fast ease-standard group-open/section:rotate-90"
        />
        <span className="font-medium text-fg">{title}</span>
        <span className="ml-auto truncate text-meta text-fg-subtle">
          {summary}
        </span>
      </summary>
      <div className="border-border-subtle border-t px-3 pt-3 pb-4">
        {children}
      </div>
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

/** The shortcut that submits, as a `Button`'s `shortcut`. */
export function modEnter() {
  const mac =
    typeof navigator !== 'undefined' &&
    /Mac|iPhone|iPad/.test(navigator.platform);
  return mac ? '⌘↵' : 'Ctrl↵';
}

/** The shortcut hint, for where a `Button`'s `shortcut` does not reach. */
export function ModEnterHint() {
  return <Kbd>{modEnter()}</Kbd>;
}
