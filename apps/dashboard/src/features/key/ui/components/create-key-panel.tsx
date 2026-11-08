import { zodResolver } from '@hookform/resolvers/zod';
import {
  Button,
  Input,
  SegmentedControl,
  SegmentedItem,
  Tab,
  TabList,
  TabPanel,
  Tabs,
  WarningIcon,
} from '@rustrak/ui';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import { useForm, useFormContext } from 'react-hook-form';
import { useTranslations } from 'use-intl';
import type { CreatedKey, LabelRef, ModelRef } from '@/shared/api/schemas';
import { Choice } from '@/shared/ui/components/choice';
import { CodeBlock, CopyButton } from '@/shared/ui/components/copy-button';
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/form';
import { LabelPicker } from '@/shared/ui/components/label-picker';
import { LimitSections } from '@/shared/ui/components/limit-sections';
import { ModelPicker } from '@/shared/ui/components/model-picker';
import {
  modEnter,
  PanelBody,
  PanelFooter,
  PanelSection,
  SidePanel,
  submitOnModEnter,
} from '@/shared/ui/components/side-panel';
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { KeysIcon } from '@/shared/ui/icons';
import { createKey } from '../../api/mutations';
import {
  type CreateKeyValues,
  createKeySchema,
  EXPIRY_OPTIONS,
  PERSONAL,
  snippets,
  toCreateKeyPayload,
} from '../../model/key-form';
import { BodySection } from './body-section';

/** A team as the key form needs it: where the key goes, what it may call. */
export interface KeyTeam {
  id: number;
  name: string;
  all_models: boolean;
  models: ModelRef[];
  /** Who a key of the team may be for; only a team's own page has them. */
  people?: { id: number; name: string }[];
}

/** The models a key of `team` (none: a personal key) may be limited to. */
export function modelChoices(
  team: KeyTeam | undefined,
  allModels: ModelRef[],
): ModelRef[] {
  return !team || team.all_models ? allModels : team.models;
}

/**
 * Creating a key: who it bills to, what it may call, and the limits folded
 * away until wanted. Once created, the same panel shows the key, once.
 */
export function CreateKeyPanel({
  open,
  onOpenChange,
  teams,
  allModels,
  labels,
  defaultTeamId,
  publicUrl,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  teams: KeyTeam[];
  allModels: ModelRef[];
  labels: LabelRef[];
  defaultTeamId?: number;
  publicUrl: string | null;
  onCreated: () => void;
}) {
  const t = useTranslations('keys');
  const tCommon = useTranslations('common');
  const money = useMoney();
  const [created, setCreated] = useState<CreatedKey | null>(null);
  const firstTeam = defaultTeamId ?? teams[0]?.id;
  const form = useForm<CreateKeyValues>({
    resolver: zodResolver(createKeySchema(t)),
    defaultValues: {
      team_id: firstTeam ? String(firstTeam) : PERSONAL,
      person_id: '',
      name: '',
      models: [],
      labels: [],
      max_budget_usd: '',
      budget_duration: 'none',
      rpm_limit: '',
      tpm_limit: '',
      max_parallel_requests: '',
      log_bodies: false,
      body_retention_days: '',
      expires: 'never',
    },
  });
  const serverErrors = useServerErrors(form, { toast: true });
  const values = form.watch();
  const personal = values.team_id === PERSONAL;
  const team = teams.find((x) => String(x.id) === values.team_id);
  const choices = modelChoices(team, allModels);

  const expiryLabels: Record<(typeof EXPIRY_OPTIONS)[number], string> = {
    never: t('expiry.never'),
    '7': t('expiry.days7'),
    '30': t('expiry.days30'),
    '90': t('expiry.days90'),
    '365': t('expiry.year'),
  };
  const close = (next: boolean) => onOpenChange(next);

  const onSubmit = form.handleSubmit(async (submitted) => {
    const result = await createKey(
      toCreateKeyPayload(submitted, new Date(), money.perUsd),
    );
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    setCreated(result.data);
    onCreated();
  });

  return (
    <SidePanel
      open={open}
      onOpenChange={close}
      onClosed={() => {
        setCreated(null);
        form.reset();
      }}
      icon={KeysIcon}
      title={created ? t('reveal.title') : t('create.title')}
      description={created ? undefined : t('create.description')}
    >
      {created ? (
        <KeyReveal
          created={created}
          model={choices[0]?.name ?? 'gpt-4o'}
          origin={publicUrl ?? window.location.origin}
          onDone={() => close(false)}
        />
      ) : (
        <Form {...form}>
          <form
            onSubmit={onSubmit}
            onKeyDown={submitOnModEnter}
            className="flex min-h-0 flex-1 flex-col"
            noValidate
          >
            <PanelBody>
              <FormField
                control={form.control}
                name="name"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t('form.name')}</FormLabel>
                    <FormControl>
                      <Input autoFocus placeholder="backend-prod" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="team_id"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t('form.owner')}</FormLabel>
                    <SegmentedControl
                      aria-label={t('form.owner')}
                      className="w-full [&>*]:flex-1"
                      value={personal ? PERSONAL : 'team'}
                      onValueChange={(next) => {
                        field.onChange(
                          next === PERSONAL ? PERSONAL : String(firstTeam),
                        );
                        form.setValue('models', []);
                        form.setValue('person_id', '');
                      }}
                    >
                      <SegmentedItem value="team" disabled={!firstTeam}>
                        {t('form.ownerTeam')}
                      </SegmentedItem>
                      <SegmentedItem value={PERSONAL}>
                        {t('form.ownerPersonal')}
                      </SegmentedItem>
                    </SegmentedControl>
                    {!personal && (
                      <Choice
                        aria-label={t('form.ownerTeam')}
                        value={field.value}
                        onChange={(value) => {
                          field.onChange(value);
                          form.setValue('models', []);
                          form.setValue('person_id', '');
                        }}
                        options={teams.map((x) => ({
                          value: String(x.id),
                          label: x.name,
                        }))}
                      />
                    )}
                    <FormDescription>
                      {personal ? t('form.personalHint') : t('form.teamHint')}
                    </FormDescription>
                  </FormItem>
                )}
              />
              {!personal && <PersonField people={team?.people} />}
              <FormField
                control={form.control}
                name="models"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t('form.models')}</FormLabel>
                    <ModelPicker
                      models={choices}
                      value={field.value}
                      onChange={field.onChange}
                      emptyMeaning={
                        team
                          ? t('form.modelsEmpty')
                          : t('form.modelsEmptyPersonal')
                      }
                    />
                    <FormMessage />
                  </FormItem>
                )}
              />
              <LabelsField labels={labels} />
              <div className="space-y-2">
                <LimitSections />
                <BodySection />
                <PanelSection
                  title={t('form.sections.expiry')}
                  summary={expiryLabels[values.expires]}
                >
                  <FormField
                    control={form.control}
                    name="expires"
                    render={({ field }) => (
                      <FormItem>
                        <Choice
                          aria-label={t('form.expires')}
                          value={field.value}
                          onChange={field.onChange}
                          options={EXPIRY_OPTIONS.map((value) => ({
                            value,
                            label: expiryLabels[value],
                          }))}
                        />
                        <FormDescription>{t('form.fromToday')}</FormDescription>
                      </FormItem>
                    )}
                  />
                </PanelSection>
              </div>
            </PanelBody>
            <PanelFooter>
              <Button
                type="button"
                variant="secondary"
                onClick={() => close(false)}
              >
                {tCommon('cancel')}
              </Button>
              <Button
                type="submit"
                variant="primary"
                shortcut={modEnter()}
                loading={form.formState.isSubmitting}
              >
                {t('create.submit')}
              </Button>
            </PanelFooter>
          </form>
        </Form>
      )}
    </SidePanel>
  );
}

/**
 * A key shown its one time: a warning, the key to copy, and how to call the
 * gateway with it. Goes inside a `SidePanel`.
 */
export function KeyReveal({
  created,
  model,
  origin,
  onDone,
}: {
  created: CreatedKey;
  model: string;
  origin: string;
  onDone: () => void;
}) {
  const t = useTranslations('keys');
  const examples = snippets(origin, created.key, model);
  return (
    <>
      <PanelBody>
        <div className="flex gap-3 rounded-lg border border-sev-warning/30 bg-sev-warning-surface p-3 text-body text-fg">
          <WarningIcon
            size="md"
            className="mt-0.5 shrink-0 text-sev-warning-fg"
          />
          <p>{t('reveal.description')}</p>
        </div>
        <div className="space-y-2">
          <p className="text-label text-fg-secondary">{created.name}</p>
          <div className="flex items-center gap-2 rounded-lg border border-border-field bg-code p-1.5 pl-3">
            <code className="flex-1 truncate font-mono text-mono text-fg-brand">
              {created.key}
            </code>
            <CopyButton text={created.key} />
          </div>
        </div>
        <div className="space-y-2">
          <p className="text-label text-fg-secondary">{t('reveal.usage')}</p>
          <Tabs defaultValue="curl">
            <TabList>
              <Tab size="sm" value="curl">
                curl
              </Tab>
              <Tab size="sm" value="python">
                Python
              </Tab>
              <Tab size="sm" value="node">
                Node
              </Tab>
              <Tab size="sm" value="anthropic">
                Claude Code
              </Tab>
            </TabList>
            <TabPanel value="curl">
              <CodeBlock code={examples.curl} />
            </TabPanel>
            <TabPanel value="python">
              <CodeBlock code={examples.python} />
            </TabPanel>
            <TabPanel value="node">
              <CodeBlock code={examples.node} />
            </TabPanel>
            <TabPanel value="anthropic">
              <CodeBlock code={examples.anthropic} />
            </TabPanel>
          </Tabs>
        </div>
      </PanelBody>
      <PanelFooter>
        <Button variant="primary" onClick={onDone}>
          {t('reveal.done')}
        </Button>
      </PanelFooter>
    </>
  );
}

/**
 * Which of the team's people the key is for. Hidden only when the people are
 * unknown (a personal key, or a page that did not load them).
 */
export function PersonField({
  people,
}: {
  people?: { id: number; name: string }[];
}) {
  const t = useTranslations('keys');
  const { control } = useFormContext<{ person_id: string }>();
  if (!people) return null;
  return (
    <FormField
      control={control}
      name="person_id"
      render={({ field }) => (
        <FormItem>
          <FormLabel>{t('form.person')}</FormLabel>
          <Choice
            aria-label={t('form.person')}
            value={field.value}
            onChange={field.onChange}
            disabled={people.length === 0}
            options={[
              { value: '', label: t('form.personNone') },
              ...people.map((p) => ({ value: String(p.id), label: p.name })),
            ]}
          />
          <FormDescription>
            {people.length === 0 ? (
              <>
                {t('form.personEmpty')}{' '}
                <Link to="/people" className="text-fg-brand hover:underline">
                  {t('form.personAdd')}
                </Link>
              </>
            ) : (
              t('form.personHint')
            )}
          </FormDescription>
        </FormItem>
      )}
    />
  );
}

/** The labels a key carries, picked from those admins defined. */
export function LabelsField({ labels }: { labels: LabelRef[] }) {
  const t = useTranslations('keys');
  const { control } = useFormContext<{ labels: number[] }>();
  return (
    <FormField
      control={control}
      name="labels"
      render={({ field }) => (
        <FormItem>
          <FormLabel>{t('form.labels')}</FormLabel>
          <LabelPicker
            labels={labels}
            value={field.value}
            onChange={field.onChange}
          />
          {labels.length > 0 && (
            <FormDescription>{t('form.labelsHint')}</FormDescription>
          )}
        </FormItem>
      )}
    />
  );
}
