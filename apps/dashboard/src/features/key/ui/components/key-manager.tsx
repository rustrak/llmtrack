import { useRouter } from '@tanstack/react-router';
import { Plus } from 'lucide-react';
import type React from 'react';
import { useState } from 'react';
import { useTranslations } from 'use-intl';
import type { ApiKey, LabelRef, ModelRef } from '@/shared/api/schemas';
import { Button } from '@/shared/ui/components/shadcn/button';
import { CreateKeyPanel, type KeyTeam, modelChoices } from './create-key-panel';
import { KeyActions } from './key-actions';
import { KeysTable } from './keys-table';

/**
 * The keys table with each key's actions. Used on its own page and inside a
 * team's page; `CreateKeyButton` goes wherever that page puts its actions.
 */
export function KeyManager({
  keys,
  teams,
  allModels,
  labels,
  teamId,
  publicUrl,
  ...table
}: {
  keys: ApiKey[];
  teams: KeyTeam[];
  allModels: ModelRef[];
  /** What a key may be labelled with. */
  labels: LabelRef[];
  teamId?: number;
  /** Where applications reach the gateway, for the code samples. */
  publicUrl: string | null;
} & Pick<
  React.ComponentProps<typeof KeysTable>,
  'sort' | 'onSort' | 'toolbar' | 'footer' | 'empty'
>) {
  const router = useRouter();
  return (
    <KeysTable
      {...table}
      keys={keys}
      showTeam={teamId === undefined}
      actions={(key) => (
        <KeyActions
          apiKey={key}
          choices={modelChoices(
            teams.find((x) => x.id === key.team_id),
            allModels,
          )}
          people={teams.find((x) => x.id === key.team_id)?.people}
          labels={labels}
          publicUrl={publicUrl}
          onChanged={() => router.invalidate()}
        />
      )}
    />
  );
}

export function CreateKeyButton({
  teams,
  allModels,
  labels,
  teamId,
  publicUrl,
}: {
  teams: KeyTeam[];
  allModels: ModelRef[];
  labels: LabelRef[];
  teamId?: number;
  publicUrl: string | null;
}) {
  const t = useTranslations('keys');
  const router = useRouter();
  const [creating, setCreating] = useState(false);
  return (
    <>
      <Button onClick={() => setCreating(true)}>
        <Plus />
        {t('create.button')}
      </Button>
      <CreateKeyPanel
        open={creating}
        onOpenChange={setCreating}
        teams={teams}
        allModels={allModels}
        labels={labels}
        defaultTeamId={teamId}
        publicUrl={publicUrl}
        onCreated={() => router.invalidate()}
      />
    </>
  );
}
