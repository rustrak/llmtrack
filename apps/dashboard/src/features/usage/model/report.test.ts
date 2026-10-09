import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SECTIONS,
  fileNameFrom,
  narrowedBy,
  parseMarkup,
  REPORT_SECTIONS,
  type ReportOptions,
  reportUrl,
  SECTION_GROUPS,
} from './report';

const options: ReportOptions = {
  format: 'pdf',
  sections: DEFAULT_SECTIONS,
  markup: '',
  showCost: false,
  client: '',
  reference: '',
  notes: '',
};
const scope = { from: '2026-09-01', to: '2026-09-30' };

describe('reportUrl', () => {
  it('sends only what differs from the defaults', () => {
    expect(reportUrl(scope, options)).toBe(
      '/api/usage/export?format=pdf&from=2026-09-01&to=2026-09-30',
    );
  });

  it('carries the filters and the choices', () => {
    const url = new URL(
      reportUrl(
        { ...scope, team_id: 3, model: 'gpt-4o' },
        {
          ...options,
          format: 'xlsx',
          sections: ['lines', 'summary'],
          markup: '12,5',
          showCost: true,
          client: '  Initech ',
          notes: 'Septiembre',
        },
      ),
      'http://x',
    );
    const q = url.searchParams;
    expect(q.get('format')).toBe('xlsx');
    expect(q.get('team_id')).toBe('3');
    expect(q.get('model')).toBe('gpt-4o');
    expect(q.get('sections')).toBe('summary,lines');
    expect(q.get('markup')).toBe('12.5');
    expect(q.get('show_cost')).toBe('true');
    expect(q.get('client')).toBe('Initech');
    expect(q.get('reference')).toBeNull();
    expect(q.get('notes')).toBe('Septiembre');
  });

  it('shows no cost without a markup to compare it with', () => {
    const url = reportUrl(scope, { ...options, showCost: true });
    expect(url).not.toContain('show_cost');
  });
});

describe('parseMarkup', () => {
  it('takes a percentage either way it is written', () => {
    expect(parseMarkup('')).toBe(0);
    expect(parseMarkup('15')).toBe(15);
    expect(parseMarkup('7,5')).toBe(7.5);
  });

  it('refuses what the server would', () => {
    expect(parseMarkup('-1')).toBeNull();
    expect(parseMarkup('5000')).toBeNull();
    expect(parseMarkup('abc')).toBeNull();
  });
});

describe('fileNameFrom', () => {
  it('reads the server name, else the fallback', () => {
    expect(
      fileNameFrom('attachment; filename="llmtrack-usage-a-b.pdf"', 'x.pdf'),
    ).toBe('llmtrack-usage-a-b.pdf');
    expect(fileNameFrom(null, 'x.pdf')).toBe('x.pdf');
  });
});

describe('narrowedBy', () => {
  it('drops the breakdowns a filter already answers', () => {
    expect(narrowedBy(scope)).toEqual([]);
    expect(
      narrowedBy({ ...scope, team_id: 1, model: 'gpt-4o', person_id: 2 }),
    ).toEqual(['teams', 'people', 'models']);
  });

  it('groups every section exactly once', () => {
    const grouped = SECTION_GROUPS.flatMap((g) => g.sections);
    expect([...grouped].sort()).toEqual([...REPORT_SECTIONS].sort());
  });
});
