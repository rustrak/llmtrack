import { Link, useRouter } from '@tanstack/react-router';
import { UserMinus } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { Person } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { Choice } from '@/shared/ui/components/choice';
import {
  ActionsCell,
  Cell,
  ColumnHead,
  DataRow,
  DataTable,
  DataTableEmpty,
} from '@/shared/ui/components/data-table';
import { Button } from '@/shared/ui/components/shadcn/button';
import { useMoney } from '@/shared/ui/hooks/use-money';
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
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        {t('people.hint')}{' '}
        <Link to="/people" className="underline underline-offset-4">
          {t('people.manage')}
        </Link>
      </p>
      {canManage && unassigned.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          <Choice
            aria-label={t('people.pick')}
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
            disabled={pending || picked === ''}
            onClick={() => {
              const person = unassigned.find((p) => String(p.id) === picked);
              if (person)
                move(person, teamId, t('people.added', { name: person.name }));
            }}
          >
            {t('people.add')}
          </Button>
        </div>
      )}
      <DataTable
        head={
          <>
            <ColumnHead>{t('people.name')}</ColumnHead>
            <ColumnHead>{t('people.email')}</ColumnHead>
            <ColumnHead className="text-right">{t('people.keys')}</ColumnHead>
            <ColumnHead className="text-right">{t('people.spend')}</ColumnHead>
            {canManage && <ColumnHead className="w-12" />}
          </>
        }
        empty={
          people.length === 0 && <DataTableEmpty title={t('people.empty')} />
        }
      >
        {people.map((person) => (
          <DataRow key={person.id}>
            <Cell className="font-medium">{person.name}</Cell>
            <Cell className="text-muted-foreground">{person.email ?? '—'}</Cell>
            <Cell className="text-right tabular-nums">{person.key_count}</Cell>
            <Cell className="text-right tabular-nums">
              {money.usd(person.spend_usd)}
            </Cell>
            {canManage && (
              <ActionsCell>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  disabled={pending}
                  aria-label={t('people.remove', { name: person.name })}
                  title={t('people.remove', { name: person.name })}
                  onClick={() =>
                    move(
                      person,
                      null,
                      t('people.removed', { name: person.name }),
                    )
                  }
                >
                  <UserMinus />
                </Button>
              </ActionsCell>
            )}
          </DataRow>
        ))}
      </DataTable>
    </div>
  );
}
