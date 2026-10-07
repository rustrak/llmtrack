import { describe, expect, it } from 'vitest';
import {
  FALLBACK_TIME_ZONE,
  formatsFor,
  resolveLocale,
  resolveTimeZone,
} from './config';

/**
 * The three decisions Next used to make inside `getRequestConfig`.
 *
 * They moved to the browser with the rest of the app, and they are the ones a
 * wrong port makes silently: a reader whose preference is ignored reads a
 * dashboard in the wrong language, and a bad zone name makes every component
 * that renders a date throw a `RangeError` rather than degrade.
 */

describe('resolveLocale', () => {
  it('prefers the language stored on the account', () => {
    expect(resolveLocale('es', ['de-DE', 'en'])).toBe('es');
  });

  it('falls back to the browser when the account has no preference', () => {
    expect(resolveLocale(null, ['de-DE', 'es'])).toBe('es');
    expect(resolveLocale(undefined, ['es-419'])).toBe('es');
  });

  it('matches on the base tag, so every es-* reaches es', () => {
    for (const tag of ['es-ES', 'es-MX', 'es-419', 'ES-ar']) {
      expect(resolveLocale(null, [tag])).toBe('es');
    }
  });

  it('takes the browser list in the order it is given', () => {
    expect(resolveLocale(null, ['en', 'es'])).toBe('en');
    expect(resolveLocale(null, ['es', 'en'])).toBe('es');
  });

  it('skips languages this app has no messages for', () => {
    expect(resolveLocale(null, ['de', 'ja', 'es'])).toBe('es');
  });

  // The server accepts any well-formed language tag by design — it has no
  // business knowing which ones this dashboard ships — so `pt-BR` can be in
  // the column with no Portuguese messages to render it with.
  it('does not trust a stored language it has no messages for', () => {
    expect(resolveLocale('pt-BR', ['es'])).toBe('es');
    expect(resolveLocale('pt-BR', [])).toBe('en');
  });

  it('answers English when nothing else matches', () => {
    expect(resolveLocale(null, [])).toBe('en');
    expect(resolveLocale('', ['de'])).toBe('en');
  });
});

describe('resolveTimeZone', () => {
  it('keeps a zone this runtime can format with', () => {
    expect(resolveTimeZone('Europe/Madrid')).toBe('Europe/Madrid');
  });

  // `Intl` throws `RangeError` on a name it does not know, and in a formatter
  // that takes down every screen showing a timestamp.
  it('refuses a zone name Intl does not know', () => {
    expect(resolveTimeZone('Mars/Olympus_Mons')).toBe(FALLBACK_TIME_ZONE);
  });

  it('falls back to UTC when the account has no zone', () => {
    expect(resolveTimeZone(null)).toBe(FALLBACK_TIME_ZONE);
    expect(resolveTimeZone(undefined)).toBe(FALLBACK_TIME_ZONE);
    expect(resolveTimeZone('')).toBe(FALLBACK_TIME_ZONE);
  });
});

describe('formatsFor', () => {
  // The zone is shown only when the time is UTC, because a UTC
  // time looks exactly like a local one and is silently hours out.
  it('labels the zone only on UTC', () => {
    expect(formatsFor('UTC').dateTime.dateTime.timeStyle).toBe('long');
    expect(formatsFor('Europe/Madrid').dateTime.dateTime.timeStyle).toBe(
      'short',
    );
    expect(formatsFor('UTC').dateTime.time).toHaveProperty('timeZoneName');
    expect(formatsFor('Europe/Madrid').dateTime.time).not.toHaveProperty(
      'timeZoneName',
    );
  });

  // A date with no time on it is not a clock reading, and "Jan 5, 2026 UTC"
  // invites the reader to wonder which day it is for them.
  it('never labels a calendar day', () => {
    expect(formatsFor('UTC').dateTime.date).not.toHaveProperty('timeZoneName');
  });

  it('names the number formats the app formats through', () => {
    const { number } = formatsFor('UTC');
    expect(number.compact.notation).toBe('compact');
    expect(number.percent.style).toBe('percent');
    // Signed, because on a delta the sign is the point.
    expect(number.percentChange.signDisplay).toBe('exceptZero');
  });
});
