import { describe, expect, it } from 'vitest';
import { resumablePath } from './return-path';

describe('resumablePath', () => {
  it('resumes a path on this origin, query included', () => {
    expect(resumablePath('/oauth/authorize?client_id=x&state=y')).toBe(
      '/oauth/authorize?client_id=x&state=y',
    );
  });

  it('never another origin', () => {
    for (const stored of [
      null,
      '',
      'https://evil.example/x',
      '//evil.example/x',
      '/\\evil.example/x',
      '/.//evil.example/x',
    ]) {
      expect(resumablePath(stored)).toBeUndefined();
    }
  });
});
