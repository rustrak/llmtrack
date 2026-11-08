import { Avatar, Button, RemoveIcon, Tooltip } from '@rustrak/ui';
import { Link, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Person } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
import {
  SettingSection,
  SettingsEmpty,
  SettingsListItem,
} from '@/shared/ui/components/setting-row';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { PeopleIcon } from '@/shared/ui/icons';
import { updatePerson } from '../../api/mutations';

/**
 * The team's people: who spends through its keys without signing in. They
 * are created on the People page and placed here from those without a team.
 */
export function PeoplePanel({
  teamId,
  people,
  unassigned,
  canManage,
}: {
  teamId: number;
  people: Person[];
  /** People without a team, to pick from. */
  unassigned: Person[];
  canManage: boolean;
}) {
  const t = useTranslations('teams');
  const router = useRouter();
  const money = useMoney();
  const [picked, setPicked] = useState('');
  const [pending, setPending] = useState(false);

  const move = async (person: Person, team: number | null, done: string) => {
    setPending(true);
    const result = await updatePerson(person.id, { team_id: team });
    setPending(false);
    if (!result.success) {
      toast.error(readableMessage(result.error));
      return;
    }
    toast.success(done);
    setPicked('');
    await router.invalidate();
  };

  return (
    <SettingSection
      title={t('detail.peopleTab')}
      description={
        <>
          {t('people.hint')}{' '}
          <Link to="/people" className="text-fg-brand hover:underline">
            {t('people.manage')}
          </Link>
        </>
      }
      actions={
        canManage &&
        unassigned.length > 0 && (
          <>
            <Choice
              aria-label={t('people.pick')}
              size="sm"
              className="w-64"
              placeholder={t('people.pick')}
              value={picked}
              onChange={setPicked}
              options={unassigned.map((p) => ({
                value: String(p.id),
                label: p.email ? `${p.name} · ${p.email}` : p.name,
              }))}
            />
            <Button
              variant="primary"
              size="sm"
              loading={pending}
              disabled={picked === ''}
              onClick={() => {
                const person = unassigned.find((p) => String(p.id) === picked);
                if (person)
                  move(
                    person,
                    teamId,
                    t('people.added', { name: person.name }),
                  );
              }}
            >
              {t('people.add')}
            </Button>
          </>
        )
      }
    >
      {people.length === 0 ? (
        <SettingsEmpty
          icon={<PeopleIcon size="2xl" />}
          title={t('people.empty')}
        />
      ) : (
        people.map((person) => (
          <SettingsListItem
            key={person.id}
            leading={<Avatar name={person.name} />}
            title={person.name}
            description={person.email ?? '—'}
            meta={
              <>
                <span>
                  {t('people.keys')}{' '}
                  <span className="font-mono text-mono-sm text-fg-secondary tabular-nums">
                    {person.key_count}
                  </span>
                </span>
                <span>
                  {t('people.spend')}{' '}
                  <span className="font-mono text-mono-sm text-fg-secondary tabular-nums">
                    {money.usd(person.spend_usd)}
                  </span>
                </span>
              </>
            }
            actions={
              canManage && (
                <Tooltip content={t('people.remove', { name: person.name })}>
                  <Button
                    variant="ghost"
                    size="sm"
                    icon={RemoveIcon}
                    disabled={pending}
                    aria-label={t('people.remove', { name: person.name })}
                    onClick={() =>
                      move(
                        person,
                        null,
                        t('people.removed', { name: person.name }),
                      )
                    }
                  />
                </Tooltip>
              )
            }
          />
        ))
      )}
    </SettingSection>
  );
}
