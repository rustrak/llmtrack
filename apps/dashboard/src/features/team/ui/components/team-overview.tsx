import type { ReactNode } from 'react';
import { useTranslations } from 'use-intl';
import type { TeamDetail, Usage } from '@/shared/api/schemas';
import { budgetShare } from '@/shared/lib/format';
import {
  DetailsList,
  Fact,
  initials,
  ModelList,
  RecordFacts,
  RecordTitle,
  SpendSummary,
  UsageLink,
  useLimitRows,
} from '@/shared/ui/components/record-overview';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { useMoney } from '@/shared/ui/hooks/use-money';

/** The team's name with its tile, as the page title. */
export function TeamTitle({ name }: { name: string }) {
  return <RecordTitle tile={initials(name)} name={name} />;
}

/** What kind of team it is, in a line of facts under the name. */
export function TeamFacts({ team }: { team: TeamDetail }) {
  const t = useTranslations('teams');
  const tRoles = useTranslations('roles');
  const money = useMoney();
  return (
    <RecordFacts>
      <Badge variant="secondary">
        {team.all_models
          ? t('allModels')
          : t('modelCount', { count: team.models.length })}
      </Badge>
      {team.my_role && (
        <Badge variant="outline">
          {t('detail.yourRole', { role: tRoles(team.my_role) })}
        </Badge>
      )}
      <Fact>
        {t('detail.createdOn', { date: money.date(team.created_at) })}
      </Fact>
    </RecordFacts>
  );
}

/** Spend over the last 30 days, and against the budget when there is one. */
export function SpendCard({
  team,
  usage,
  chart,
}: {
  team: TeamDetail;
  usage: Usage;
  chart: ReactNode;
}) {
  const t = useTranslations('teams');
  const money = useMoney();
  const share = budgetShare(team.spend_usd, team.max_budget_usd);
  return (
    <SpendSummary
      label={t('detail.spend30d')}
      total={money.usd(usage.totals.cost_usd)}
      hint={t('detail.requests30d', {
        count: money.compact(usage.totals.requests),
      })}
      link={
        <UsageLink search={{ team: team.id }} label={t('detail.openUsage')} />
      }
      budget={
        share !== null && team.max_budget_usd !== null
          ? {
              text: t('detail.ofBudget', {
                spent: money.usd(team.spend_usd),
                budget: money.usd(team.max_budget_usd),
              }),
              reset: team.budget_reset_at
                ? t('detail.resets', { date: money.date(team.budget_reset_at) })
                : t('detail.lifetime'),
              share,
            }
          : undefined
      }
      chart={chart}
    />
  );
}

/** How the team is set up. */
export function DetailsCard({ team }: { team: TeamDetail }) {
  const t = useTranslations('teams');
  const rows = useLimitRows(team);
  const access = team.all_models ? (
    t('allModels')
  ) : team.models.length === 0 ? (
    <span className="font-normal text-destructive">{t('detail.noModels')}</span>
  ) : (
    <ModelList models={team.models} />
  );
  return (
    <DetailsList
      title={t('detail.details')}
      rows={[...rows, [t('detail.modelAccess'), access]]}
    />
  );
}
