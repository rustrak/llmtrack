import { createFileRoute, Link, useRouter } from '@tanstack/react-router';
import { KeyRound } from 'lucide-react';
import { useTranslations } from 'use-intl';
import { getKey } from '@/features/key/api/queries';
import { modelChoices } from '@/features/key/ui/components/create-key-panel';
import { KeyActions } from '@/features/key/ui/components/key-actions';
import { KeyStatus } from '@/features/key/ui/components/keys-table';
import { listLabels } from '@/features/label/api/queries';
import { listModels } from '@/features/model/api/queries';
import { getSettings } from '@/features/settings/api/queries';
import { listTeams } from '@/features/team/api/queries';
import { getUsage } from '@/features/usage/api/queries';
import { rangeFor } from '@/features/usage/model/range';
import { groupTotals } from '@/features/usage/model/series';
import {
  StackedBars,
  useBucketLabel,
} from '@/features/usage/ui/components/usage-charts';
import { UsageRanking } from '@/features/usage/ui/components/usage-ranking';
import { unwrap } from '@/shared/api/http';
import type { ApiKey } from '@/shared/api/schemas';
import { translator } from '@/shared/i18n/intl';
import { budgetShare } from '@/shared/lib/format';
import { MAX_PAGE_SIZE } from '@/shared/lib/list-params';
import { LabelBadge } from '@/shared/ui/components/label-badge';
import { Page } from '@/shared/ui/components/page';
import {
  DetailsList,
  Fact,
  ModelList,
  RecordFacts,
  RecordTitle,
  SpendSummary,
  UsageLink,
  useLimitRows,
} from '@/shared/ui/components/record-overview';
import { Badge } from '@/shared/ui/components/shadcn/badge';
import { useMoney } from '@/shared/ui/hooks/use-money';

export const Route = createFileRoute('/_authenticated/keys/$keyId')({
  loader: async ({ params }) => {
    const id = Number(params.keyId);
    const [key, usage, teams, models, labels, settings] = await Promise.all([
      getKey(id),
      getUsage({ ...rangeFor('30d'), key_id: id }),
      listTeams({ per_page: MAX_PAGE_SIZE }),
      listModels({ per_page: MAX_PAGE_SIZE }),
      listLabels({ per_page: MAX_PAGE_SIZE }),
      getSettings(),
    ]);
    return {
      apiKey: unwrap(key),
      usage: unwrap(usage),
      teams: unwrap(teams).data,
      models: unwrap(models).data,
      labels: unwrap(labels).data,
      publicUrl: unwrap(settings).public_url,
    };
  },
  head: ({ loaderData }) => ({
    meta: [
      {
        title: translator('keys')('meta.detailTitle', {
          name: loaderData?.apiKey.name ?? '',
        }),
      },
    ],
  }),
  component: KeyPage,
});

function KeyPage() {
  const t = useTranslations('keys');
  const tUsage = useTranslations('usage');
  const { apiKey, usage, teams, models, labels, publicUrl } =
    Route.useLoaderData();
  const router = useRouter();
  const money = useMoney();
  const bucketLabel = useBucketLabel('day');
  const allModels = models.map(({ id, name }) => ({ id, name }));
  const share = budgetShare(apiKey.spend_usd, apiKey.max_budget_usd);

  return (
    <Page
      title={
        <RecordTitle
          tile={<KeyRound />}
          name={apiKey.name}
          aside={apiKey.key_hint}
        />
      }
      description={<KeyFacts apiKey={apiKey} />}
      actions={
        <KeyActions
          apiKey={apiKey}
          choices={modelChoices(
            teams.find((x) => x.id === apiKey.team_id),
            allModels,
          )}
          labels={labels}
          publicUrl={publicUrl}
          onChanged={() => router.invalidate()}
          onRevoked={() => router.navigate({ to: '/keys' })}
        />
      }
    >
      <div className="grid gap-4 lg:grid-cols-3">
        <SpendSummary
          label={t('detail.spend30d')}
          total={money.usd(usage.totals.cost_usd)}
          hint={t('detail.requests30d', {
            count: money.compact(usage.totals.requests),
          })}
          link={
            <UsageLink
              search={{ key: apiKey.id, team: apiKey.team_id ?? undefined }}
              label={t('detail.openUsage')}
            />
          }
          budget={
            share !== null && apiKey.max_budget_usd !== null
              ? {
                  text: t('detail.ofBudget', {
                    spent: money.usd(apiKey.spend_usd),
                    budget: money.usd(apiKey.max_budget_usd),
                  }),
                  reset: apiKey.budget_reset_at
                    ? t('detail.resets', {
                        date: money.date(apiKey.budget_reset_at),
                      })
                    : t('detail.lifetime'),
                  share,
                }
              : undefined
          }
          chart={
            <StackedBars
              height="h-44"
              rows={usage.daily.map((d) => ({
                bucket: d.day,
                spend: d.cost_usd,
              }))}
              series={[
                {
                  key: 'spend',
                  label: tUsage('spend'),
                  color: 'var(--chart-1)',
                },
              ]}
              format={(v) => money.usd(v)}
              bucketLabel={bucketLabel}
            />
          }
        />
        <KeyDetails apiKey={apiKey} />
      </div>
      <section className="space-y-3">
        <h2 className="text-sm font-medium">{t('detail.byModel')}</h2>
        <UsageRanking
          groups={groupTotals(usage.series, 'spend')}
          metric="spend"
          groupLabel={t('table.models')}
          nameOf={(g) => g.label ?? g.key}
          empty={tUsage('table.empty')}
        />
      </section>
    </Page>
  );
}

/** State, who it bills to, who it is for, and its history, under the name. */
function KeyFacts({ apiKey }: { apiKey: ApiKey }) {
  const t = useTranslations('keys');
  const money = useMoney();
  const expired =
    apiKey.expires_at !== null && Date.parse(apiKey.expires_at) <= Date.now();
  return (
    <RecordFacts>
      {apiKey.blocked ? (
        <Badge variant="destructive">{t('blocked')}</Badge>
      ) : expired ? (
        <Badge variant="destructive">{t('expired')}</Badge>
      ) : (
        <Badge variant="secondary">{t('detail.active')}</Badge>
      )}
      {apiKey.team_id !== null ? (
        <Badge
          variant="outline"
          render={
            <Link
              to="/teams/$teamId"
              params={{ teamId: String(apiKey.team_id) }}
            />
          }
        >
          {apiKey.team_name}
        </Badge>
      ) : (
        <Badge variant="outline">
          {t('detail.personalOf', { email: apiKey.owner_email ?? '—' })}
        </Badge>
      )}
      {apiKey.labels.map((label) => (
        <LabelBadge key={label.id} label={label} />
      ))}
      {apiKey.person_name && (
        <Badge variant="outline">{apiKey.person_name}</Badge>
      )}
      <Fact>
        {apiKey.created_by_email
          ? t('detail.createdBy', {
              date: money.date(apiKey.created_at),
              email: apiKey.created_by_email,
            })
          : t('detail.createdOn', { date: money.date(apiKey.created_at) })}
      </Fact>
      <Fact>
        {apiKey.last_used_at
          ? t('detail.lastUsedAt', {
              date: money.dateTime(apiKey.last_used_at),
            })
          : t('detail.neverUsed')}
      </Fact>
    </RecordFacts>
  );
}

/** How the key is set up: limits, expiry and what it may call. */
function KeyDetails({ apiKey }: { apiKey: ApiKey }) {
  const t = useTranslations('keys');
  const rows = useLimitRows(apiKey);
  return (
    <DetailsList
      title={t('detail.details')}
      rows={[
        ...rows,
        [t('table.expires'), <KeyStatus key="expires" apiKey={apiKey} />],
        [
          t('table.models'),
          apiKey.models.length > 0 ? (
            <ModelList models={apiKey.models} />
          ) : apiKey.team_id === null ? (
            t('allModels')
          ) : (
            t('allTeamModels')
          ),
        ],
      ]}
    />
  );
}
