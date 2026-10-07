import { useState } from 'react';
import { useTranslations } from 'use-intl';
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from './shadcn/alert-dialog';
import { Button } from './shadcn/button';

/**
 * Asks before something that cannot be undone. `onConfirm` resolves to an
 * error message, or `null` when it worked and the dialog may close.
 */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  onConfirm: () => Promise<string | null>;
}) {
  const t = useTranslations('common');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        setError(null);
        onOpenChange(next);
      }}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        {error && <p className="text-sm text-destructive">{error}</p>}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={pending}>
            {t('cancel')}
          </AlertDialogCancel>
          <Button
            variant="destructive"
            disabled={pending}
            onClick={async () => {
              setPending(true);
              const failure = await onConfirm();
              setPending(false);
              if (failure) setError(failure);
              else onOpenChange(false);
            }}
          >
            {pending ? t('working') : confirmLabel}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
