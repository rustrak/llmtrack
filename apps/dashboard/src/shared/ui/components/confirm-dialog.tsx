import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
} from '@rustrak/ui';
import { useState } from 'react';
import { useTranslations } from 'use-intl';

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
    <Dialog
      alert
      open={open}
      onOpenChange={(next) => {
        if (pending) return;
        setError(null);
        onOpenChange(next);
      }}
    >
      <DialogHeader
        title={title}
        description={description}
        dismissible={false}
      />
      {error && (
        <DialogBody>
          <p role="alert" className="text-hint text-danger-fg">
            {error}
          </p>
        </DialogBody>
      )}
      <DialogFooter>
        <DialogClose
          disabled={pending}
          render={<Button variant="secondary">{t('cancel')}</Button>}
        />
        <Button
          variant="danger-primary"
          loading={pending}
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
      </DialogFooter>
    </Dialog>
  );
}
