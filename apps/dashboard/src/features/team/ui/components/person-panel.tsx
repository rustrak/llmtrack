import { Contact } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import {
  PanelBody,
  PanelFooter,
  SidePanel,
} from '@/shared/ui/components/side-panel';
import { createPerson } from '../../api/mutations';

/**
 * Adding someone, straight into one of the teams the reader manages or,
 * for an admin, without a team until a team picks them.
 */
export function PersonPanel({
  open,
  teams,
  allowNoTeam,
  defaultTeamId,
  onClose,
  onSaved,
}: {
  open: boolean;
  teams: { id: number; name: string }[];
  allowNoTeam: boolean;
  defaultTeamId?: number;
  onClose: () => void;
  onSaved: () => void;
}) {
  const t = useTranslations('people');
  const tCommon = useTranslations('common');
  const [teamId, setTeamId] = useState(
    String(defaultTeamId ?? (allowNoTeam ? '' : (teams[0]?.id ?? ''))),
  );
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  return (
    <SidePanel
      open={open}
      onOpenChange={(next) => !next && onClose()}
      onClosed={() => {
        setName('');
        setEmail('');
        setError(null);
      }}
      icon={Contact}
      title={t('create.title')}
      description={t('create.description')}
    >
      <form
        className="flex min-h-0 flex-1 flex-col"
        onSubmit={async (event) => {
          event.preventDefault();
          setPending(true);
          const result = await createPerson({
            name,
            email: email.trim() || null,
            team_id: teamId ? Number(teamId) : null,
          });
          setPending(false);
          if (!result.success) {
            setError(readableMessage(result.error));
            return;
          }
          toast.success(t('create.added', { name: result.data.name }));
          onSaved();
          onClose();
        }}
      >
        <PanelBody>
          <div className="space-y-1.5">
            <Label>{t('table.team')}</Label>
            <Choice
              aria-label={t('table.team')}
              value={teamId}
              onChange={setTeamId}
              options={[
                ...(allowNoTeam ? [{ value: '', label: t('noTeam') }] : []),
                ...teams.map((x) => ({ value: String(x.id), label: x.name })),
              ]}
            />
            <p className="text-xs text-muted-foreground">
              {t('create.teamHint')}
            </p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="person-name">{t('table.name')}</Label>
            <Input
              id="person-name"
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              required
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="person-email">{t('create.email')}</Label>
            <Input
              id="person-email"
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
        </PanelBody>
        <PanelFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {tCommon('cancel')}
          </Button>
          <Button
            type="submit"
            disabled={
              pending || (!teamId && !allowNoTeam) || name.trim() === ''
            }
          >
            {t('create.submit')}
          </Button>
        </PanelFooter>
      </form>
    </SidePanel>
  );
}
