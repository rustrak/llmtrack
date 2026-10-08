import { Download, FileText, Languages, Loader2 } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import { useLocale, useTranslations } from 'use-intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { Button } from '@/shared/ui/components/shadcn/button';
import { Checkbox } from '@/shared/ui/components/shadcn/checkbox';
import { Input } from '@/shared/ui/components/shadcn/input';
import { Label } from '@/shared/ui/components/shadcn/label';
import { Switch } from '@/shared/ui/components/shadcn/switch';
import {
  Tabs,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import { Textarea } from '@/shared/ui/components/shadcn/textarea';
import {
  PanelBody,
  PanelFooter,
  PanelSection,
  SidePanel,
} from '@/shared/ui/components/side-panel';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { fetchReport } from '../../api/queries';
import {
  DEFAULT_SECTIONS,
  fileNameFrom,
  narrowedBy,
  parseMarkup,
  type ReportFormat,
  type ReportOptions,
  type ReportScope,
  type ReportSection,
  reportUrl,
  SECTION_GROUPS,
} from '../../model/report';

/**
 * Builds a report for the usage page's filters, in the panel every record
 * is created in: a PDF to hand a client, or a workbook to work with.
 * Filtered to one team, that team is the client, and breakdowns the
 * filters already answer are not offered. Language and currency are the
 * account's.
 */
export function ReportPanel({
  open,
  onOpenChange,
  scope,
  scopeLabel,
  teamName,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  scope: ReportScope;
  /** The filters in words: what the report covers. */
  scopeLabel: string;
  teamName?: string;
}) {
  const t = useTranslations('usage');
  const locale = useLocale();
  const { currency } = useMoney();
  const narrowed = narrowedBy(scope);
  const groups = SECTION_GROUPS.map((g) => ({
    key: g.key,
    sections: g.sections.filter((s) => !narrowed.includes(s)),
  })).filter((g) => g.sections.length > 0);
  const offered = groups.flatMap((g) => g.sections);
  const initial = (): ReportOptions => ({
    format: 'pdf',
    sections: DEFAULT_SECTIONS.filter((s) => !narrowed.includes(s)),
    markup: '',
    showCost: false,
    client: teamName ?? '',
    reference: '',
    notes: '',
  });
  const [options, setOptions] = useState<ReportOptions>(initial);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<ReportOptions>) =>
    setOptions((prev) => ({ ...prev, ...patch }));
  const markup = parseMarkup(options.markup);
  const toggle = (section: ReportSection, on: boolean) =>
    set({
      sections: on
        ? [...options.sections, section]
        : options.sections.filter((s) => s !== section),
    });

  const download = async () => {
    setPending(true);
    setError(null);
    const result = await fetchReport(reportUrl(scope, options));
    setPending(false);
    if (!result.success) {
      setError(readableMessage(result.error));
      return;
    }
    const link = document.createElement('a');
    link.href = URL.createObjectURL(result.data.blob);
    link.download = fileNameFrom(
      result.data.disposition,
      `llmtrack-usage-${scope.from}-${scope.to}.${options.format}`,
    );
    link.click();
    URL.revokeObjectURL(link.href);
    toast.success(t('report.ready'));
    onOpenChange(false);
  };

  const amountsSummary = markup
    ? [
        t('report.markupSummary', { value: options.markup.trim() }),
        options.showCost && t('report.costShown'),
      ]
        .filter(Boolean)
        .join(' · ')
    : t('report.atCost');
  const notesSummary = options.notes.trim()
    ? options.notes.trim().slice(0, 40)
    : t('report.noNotes');

  return (
    <SidePanel
      open={open}
      onOpenChange={onOpenChange}
      onClosed={() => {
        setOptions(initial());
        setError(null);
      }}
      icon={FileText}
      title={t('report.title')}
      description={scopeLabel}
    >
      <PanelBody>
        <div className="space-y-2">
          <Label>{t('report.format')}</Label>
          <Tabs
            value={options.format}
            onValueChange={(next) => set({ format: next as ReportFormat })}
          >
            <TabsList className="w-full">
              <TabsTrigger value="pdf">
                {t('report.formats.pdf.label')}
              </TabsTrigger>
              <TabsTrigger value="xlsx">
                {t('report.formats.xlsx.label')}
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <p className="text-xs text-muted-foreground">
            {t(`report.formats.${options.format}.hint`)}
          </p>
        </div>

        <div className="space-y-2">
          <Label htmlFor="report-client">{t('report.client')}</Label>
          <Input
            id="report-client"
            maxLength={200}
            placeholder={t('report.clientPlaceholder')}
            value={options.client}
            onChange={(e) => set({ client: e.target.value })}
          />
          {teamName && options.client === teamName && (
            <p className="text-xs text-muted-foreground">
              {t('report.clientFromTeam')}
            </p>
          )}
        </div>

        <div className="space-y-2">
          <Label htmlFor="report-reference">{t('report.reference')}</Label>
          <Input
            id="report-reference"
            maxLength={200}
            placeholder={t('report.referencePlaceholder')}
            value={options.reference}
            onChange={(e) => set({ reference: e.target.value })}
          />
        </div>

        <div className="space-y-3">
          <PanelSection
            title={t('report.amounts')}
            summary={amountsSummary}
            forceOpen={markup === null}
          >
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="report-markup">{t('report.markup')}</Label>
                <div className="relative">
                  <Input
                    id="report-markup"
                    inputMode="decimal"
                    placeholder="0"
                    className="pr-8"
                    aria-invalid={markup === null}
                    value={options.markup}
                    onChange={(e) => set({ markup: e.target.value })}
                  />
                  <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-muted-foreground">
                    %
                  </span>
                </div>
                <p
                  className={
                    markup === null
                      ? 'text-xs text-destructive'
                      : 'text-xs text-muted-foreground'
                  }
                >
                  {markup === null
                    ? t('report.markupInvalid')
                    : t('report.markupHint')}
                </p>
              </div>
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-0.5">
                  <Label htmlFor="report-show-cost">
                    {t('report.showCost')}
                  </Label>
                  <p className="text-xs text-muted-foreground">
                    {t('report.showCostHint')}
                  </p>
                </div>
                <Switch
                  id="report-show-cost"
                  checked={options.showCost && !!markup}
                  disabled={!markup}
                  onCheckedChange={(next) => set({ showCost: next })}
                />
              </div>
            </div>
          </PanelSection>

          <PanelSection
            title={t('report.sections')}
            summary={t('report.selected', {
              count: options.sections.length,
              total: offered.length,
            })}
            forceOpen={options.sections.length === 0}
          >
            <div className="space-y-4">
              <div className="flex gap-1">
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={() => set({ sections: [...offered] })}
                >
                  {t('report.all')}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="xs"
                  onClick={() => set({ sections: [] })}
                >
                  {t('report.none')}
                </Button>
              </div>
              {groups.map((group) => (
                <div key={group.key} className="space-y-1.5">
                  <p className="text-xs font-medium text-muted-foreground">
                    {t(`report.groups.${group.key}`)}
                  </p>
                  <div className="divide-y rounded-lg border">
                    {group.sections.map((section) => (
                      <label
                        key={section}
                        className="flex cursor-pointer items-center gap-3 px-3 py-2 text-sm hover:bg-muted/50"
                      >
                        <Checkbox
                          checked={options.sections.includes(section)}
                          onCheckedChange={(next) =>
                            toggle(section, next === true)
                          }
                        />
                        <span>{t(`report.section.${section}.label`)}</span>
                      </label>
                    ))}
                  </div>
                </div>
              ))}
              {narrowed.length > 0 && (
                <p className="text-xs text-muted-foreground">
                  {t('report.narrowed')}
                </p>
              )}
            </div>
          </PanelSection>

          <PanelSection title={t('report.notes')} summary={notesSummary}>
            <Textarea
              aria-label={t('report.notes')}
              maxLength={2000}
              rows={4}
              placeholder={t('report.notesPlaceholder')}
              value={options.notes}
              onChange={(e) => set({ notes: e.target.value })}
            />
          </PanelSection>
        </div>

        {error && <p className="text-sm text-destructive">{error}</p>}
      </PanelBody>

      <PanelFooter>
        <span className="mr-auto flex min-w-0 items-center gap-1.5 truncate text-xs text-muted-foreground">
          <Languages className="size-3.5 shrink-0" />
          {t('report.profile', {
            language: t(
              `report.language.${locale.startsWith('es') ? 'es' : 'en'}`,
            ),
            currency,
          })}
        </span>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t('report.cancel')}
        </Button>
        <Button
          onClick={download}
          disabled={pending || markup === null || options.sections.length === 0}
        >
          {pending ? <Loader2 className="animate-spin" /> : <Download />}
          {pending ? t('report.working') : t('report.download')}
        </Button>
      </PanelFooter>
    </SidePanel>
  );
}
