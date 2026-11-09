import {
  Button,
  Checkbox,
  ExportIcon,
  Field,
  FieldHint,
  FieldLabel,
  Input,
  NetworkIcon,
  SegmentedControl,
  SegmentedItem,
  Switch,
  Textarea,
} from '@rustrak/ui';
import { useState } from 'react';
import { toast } from 'sonner';
import { useLocale, useTranslations } from 'use-intl';
import { readableMessage } from '@/shared/lib/form-errors';
import { Caption } from '@/shared/ui/components/page';
import {
  PanelBody,
  PanelFooter,
  PanelSection,
  SidePanel,
} from '@/shared/ui/components/side-panel';
import { useMoney } from '@/shared/ui/hooks/use-money';
import { UsageIcon } from '@/shared/ui/icons';
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
      icon={UsageIcon}
      title={t('report.title')}
      description={scopeLabel}
    >
      <PanelBody>
        <Field>
          <FieldLabel>{t('report.format')}</FieldLabel>
          <SegmentedControl
            aria-label={t('report.format')}
            value={options.format}
            onValueChange={(next) => set({ format: next as ReportFormat })}
            className="w-full"
          >
            <SegmentedItem value="pdf" className="flex-1">
              {t('report.formats.pdf.label')}
            </SegmentedItem>
            <SegmentedItem value="xlsx" className="flex-1">
              {t('report.formats.xlsx.label')}
            </SegmentedItem>
          </SegmentedControl>
          <FieldHint>{t(`report.formats.${options.format}.hint`)}</FieldHint>
        </Field>

        <Field>
          <FieldLabel>{t('report.client')}</FieldLabel>
          <Input
            maxLength={200}
            placeholder={t('report.clientPlaceholder')}
            value={options.client}
            onChange={(e) => set({ client: e.target.value })}
          />
          {teamName && options.client === teamName && (
            <FieldHint>{t('report.clientFromTeam')}</FieldHint>
          )}
        </Field>

        <Field>
          <FieldLabel>{t('report.reference')}</FieldLabel>
          <Input
            maxLength={200}
            placeholder={t('report.referencePlaceholder')}
            value={options.reference}
            onChange={(e) => set({ reference: e.target.value })}
          />
        </Field>

        <div className="flex flex-col gap-3">
          <PanelSection
            title={t('report.amounts')}
            summary={amountsSummary}
            forceOpen={markup === null}
          >
            <div className="flex flex-col gap-4">
              <Field invalid={markup === null}>
                <FieldLabel>{t('report.markup')}</FieldLabel>
                <Input
                  inputMode="decimal"
                  placeholder="0"
                  numeric
                  invalid={markup === null}
                  value={options.markup}
                  onChange={(e) => set({ markup: e.target.value })}
                  action={
                    <span className="pr-1 text-fg-subtle text-meta">%</span>
                  }
                />
                <FieldHint
                  className={markup === null ? 'text-danger-fg' : undefined}
                >
                  {markup === null
                    ? t('report.markupInvalid')
                    : t('report.markupHint')}
                </FieldHint>
              </Field>
              <div className="flex items-start justify-between gap-4">
                <div className="flex flex-col gap-0.5">
                  <span className="text-fg text-label">
                    {t('report.showCost')}
                  </span>
                  <span className="text-fg-subtle text-hint">
                    {t('report.showCostHint')}
                  </span>
                </div>
                <Switch
                  aria-label={t('report.showCost')}
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
            <div className="flex flex-col gap-4">
              <div className="flex gap-1">
                <Button
                  type="button"
                  size="xs"
                  onClick={() => set({ sections: [...offered] })}
                >
                  {t('report.all')}
                </Button>
                <Button
                  type="button"
                  size="xs"
                  onClick={() => set({ sections: [] })}
                >
                  {t('report.none')}
                </Button>
              </div>
              {groups.map((group) => (
                <div key={group.key} className="flex flex-col gap-1.5">
                  <Caption>{t(`report.groups.${group.key}`)}</Caption>
                  <div className="divide-y divide-border-divider rounded-lg border border-border-subtle">
                    {group.sections.map((section) => {
                      const label = t(`report.section.${section}.label`);
                      return (
                        <label
                          key={section}
                          className="flex cursor-pointer items-center gap-3 px-3 py-2 text-body text-fg transition-[background-color] duration-fast hover:bg-surface-hover"
                        >
                          <Checkbox
                            aria-label={label}
                            checked={options.sections.includes(section)}
                            onCheckedChange={(next) =>
                              toggle(section, next === true)
                            }
                          />
                          <span>{label}</span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              ))}
              {narrowed.length > 0 && (
                <p className="text-fg-subtle text-hint">
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

        {error && <p className="text-danger-fg text-hint">{error}</p>}
      </PanelBody>

      <PanelFooter>
        <span className="mr-auto flex min-w-0 items-center gap-1.5 truncate text-fg-subtle text-meta">
          <NetworkIcon size="sm" className="shrink-0" />
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
          variant="primary"
          icon={ExportIcon}
          loading={pending}
          onClick={download}
          disabled={markup === null || options.sections.length === 0}
        >
          {pending ? t('report.working') : t('report.download')}
        </Button>
      </PanelFooter>
    </SidePanel>
  );
}
