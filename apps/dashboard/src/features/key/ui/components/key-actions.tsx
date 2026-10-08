import { Link } from '@tanstack/react-router';
import { KeyRound, MoreHorizontal } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type {
  ApiKey,
  CreatedKey,
  LabelRef,
  ModelRef,
} from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { ConfirmDialog } from '@/shared/ui/components/confirm-dialog';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/shared/ui/components/shadcn/dropdown-menu';
import { SidePanel } from '@/shared/ui/components/side-panel';
import { regenerateKey, revokeKey, setKeyBlocked } from '../../api/mutations';
import { KeyReveal } from './create-key-panel';
import { EditKeyPanel } from './edit-key-panel';

type Pending = 'edit' | 'regenerate' | 'revoke' | null;

/**
 * Everything one can do to a key: details, edit,
 * regenerate, block or unblock, revoke. Used in the table and on the key's
 * own page.
 */
export function KeyActions({
  apiKey,
  choices,
  people,
  labels,
  publicUrl,
  onChanged,
  onRevoked = onChanged,
}: {
  apiKey: ApiKey;
  choices: ModelRef[];
  people?: { id: number; name: string }[];
  labels: LabelRef[];
  publicUrl: string | null;
  onChanged: () => void;
  onRevoked?: () => void;
}) {
  const t = useTranslations('keys');
  const [pending, setPending] = useState<Pending>(null);
  const [regenerated, setRegenerated] = useState<CreatedKey | null>(null);
  const close = (open: boolean) => !open && setPending(null);

  const toggleBlocked = async () => {
    const result = await setKeyBlocked(apiKey.id, !apiKey.blocked);
    if (!result.success) {
      toast.error(readableMessage(result.error));
      return;
    }
    toast.success(
      t(apiKey.blocked ? 'blockDialog.unblocked' : 'blockDialog.blocked', {
        name: apiKey.name,
      }),
    );
    onChanged();
  };

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={t('actionsFor', { name: apiKey.name })}
            />
          }
        >
          <MoreHorizontal />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem
            render={
              <Link to="/keys/$keyId" params={{ keyId: String(apiKey.id) }} />
            }
          >
            {t('details')}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setPending('edit')}>
            {t('editAction')}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setPending('regenerate')}>
            {t('regenerate')}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={toggleBlocked}>
            {apiKey.blocked ? t('unblock') : t('block')}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            className="text-destructive"
            onClick={() => setPending('revoke')}
          >
            {t('revoke')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <EditKeyPanel
        open={pending === 'edit'}
        apiKey={apiKey}
        choices={choices}
        people={people}
        labels={labels}
        onClose={() => setPending(null)}
        onSaved={onChanged}
      />
      <ConfirmDialog
        open={pending === 'regenerate'}
        onOpenChange={close}
        title={t('regenerateDialog.title', { name: apiKey.name })}
        description={t('regenerateDialog.description')}
        confirmLabel={t('regenerateDialog.confirm')}
        onConfirm={async () => {
          const result = await regenerateKey(apiKey.id);
          if (!result.success) return readableMessage(result.error);
          setRegenerated(result.data);
          onChanged();
          return null;
        }}
      />
      <ConfirmDialog
        open={pending === 'revoke'}
        onOpenChange={close}
        title={t('revokeDialog.title', { name: apiKey.name })}
        description={t('revokeDialog.description')}
        confirmLabel={t('revokeDialog.confirm')}
        onConfirm={async () => {
          const result = await revokeKey(apiKey.id);
          if (!result.success) return readableMessage(result.error);
          toast.success(t('revokeDialog.done', { name: apiKey.name }));
          onRevoked();
          return null;
        }}
      />
      <SidePanel
        open={regenerated !== null}
        onOpenChange={(open) => !open && setRegenerated(null)}
        icon={KeyRound}
        title={t('reveal.title')}
      >
        {regenerated && (
          <KeyReveal
            created={regenerated}
            model={choices[0]?.name ?? 'gpt-4o'}
            origin={publicUrl ?? window.location.origin}
            onDone={() => setRegenerated(null)}
          />
        )}
      </SidePanel>
    </>
  );
}
