import { zodResolver } from '@hookform/resolvers/zod';
import { Users } from 'lucide-react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { ModelRef, Team } from '@/shared/api/schemas';
import { LimitSections } from '@/shared/ui/components/limit-sections';
import { ModelPicker } from '@/shared/ui/components/model-picker';
import { Button } from '@/shared/ui/components/shadcn/button';
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from '@/shared/ui/components/shadcn/form';
import { Input } from '@/shared/ui/components/shadcn/input';
import {
  Tabs,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import {
  ModEnterHint,
  PanelBody,
  PanelFooter,
  SidePanel,
  submitOnModEnter,
} from '@/shared/ui/components/side-panel';
import { useServerErrors } from '@/shared/ui/hooks/use-form-errors';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { createTeam, updateTeam } from '../../api/mutations';
import {
  type TeamFormValues,
  teamDefaults,
  teamFormSchema,
  toTeamPayload,
} from '../../model/team-form';

interface TeamFormProps {
  team?: Team;
  models: ModelRef[];
  onClose: () => void;
  onSaved: (id: number) => void;
}

/**
 * Creates a team, or edits one when `team` is given, in a `SidePanel`.
 * Mounted all along and driven by `open`; the form mounts with each
 * opening, so it starts from `team` as it is.
 */
export function TeamPanel({
  open,
  ...props
}: TeamFormProps & { open: boolean }) {
  const t = useTranslations('teams');
  const { team, onClose } = props;
  return (
    <SidePanel
      open={open}
      onOpenChange={(next) => !next && onClose()}
      icon={Users}
      title={
        team
          ? t('dialog.editTitle', { name: team.name })
          : t('dialog.createTitle')
      }
      description={t('dialog.description')}
    >
      <TeamForm key={team?.id ?? 'new'} {...props} />
    </SidePanel>
  );
}

function TeamForm({ team, models, onClose, onSaved }: TeamFormProps) {
  const t = useTranslations('teams');
  const tCommon = useTranslations('common');
  const { perUsd } = useMoney();
  const form = useForm<TeamFormValues>({
    resolver: zodResolver(teamFormSchema(t)),
    defaultValues: teamDefaults(team, perUsd),
  });
  const allModels = form.watch('all_models');

  const serverErrors = useServerErrors(form, { toast: true });

  const onSubmit = form.handleSubmit(async (values) => {
    const payload = toTeamPayload(values, perUsd);
    const result = team
      ? await updateTeam(team.id, payload)
      : await createTeam(payload);
    if (!result.success) {
      serverErrors(result.error);
      return;
    }
    toast.success(
      team
        ? t('dialog.updated', { name: payload.name })
        : t('dialog.created', { name: payload.name }),
    );
    onSaved(result.data.id);
    onClose();
  });

  return (
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
                  <Input autoFocus placeholder="Acme Corp" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          <FormField
            control={form.control}
            name="all_models"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t('form.models')}</FormLabel>
                <Tabs
                  value={field.value ? 'all' : 'restricted'}
                  onValueChange={(next) => field.onChange(next === 'all')}
                >
                  <TabsList className="w-full">
                    <TabsTrigger value="all">{t('access.all')}</TabsTrigger>
                    <TabsTrigger value="restricted">
                      {t('access.restricted')}
                    </TabsTrigger>
                  </TabsList>
                </Tabs>
                <FormDescription>
                  {field.value
                    ? t('form.allModelsHint')
                    : t('form.someModelsHint')}
                </FormDescription>
              </FormItem>
            )}
          />
          {!allModels && (
            <FormField
              control={form.control}
              name="models"
              render={({ field }) => (
                <FormItem>
                  <ModelPicker
                    models={models}
                    value={field.value}
                    onChange={field.onChange}
                    emptyMeaning={t('form.modelsNone')}
                  />
                  <FormMessage />
                </FormItem>
              )}
            />
          )}
          <div className="space-y-2">
            <LimitSections budgetHint={t('form.budgetHint')} />
          </div>
        </PanelBody>
        <PanelFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {tCommon('cancel')}
          </Button>
          <Button type="submit" disabled={form.formState.isSubmitting}>
            {team ? t('dialog.save') : t('dialog.create')}
            <ModEnterHint />
          </Button>
        </PanelFooter>
      </form>
    </Form>
  );
}
