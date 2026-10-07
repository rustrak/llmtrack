import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import { API_PREFIXES } from '@/shared/config/api-prefixes';

const src = join(import.meta.dirname, '..');

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return files(path);
    return /\.tsx?$/.test(name) && !name.endsWith('.gen.ts') ? [path] : [];
  });
}

function imports(path: string): string[] {
  const code = readFileSync(path, 'utf8');
  return [...code.matchAll(/from '(@\/[^']+)'/g)].map((m) => m[1]);
}

const all = files(src).map((path) => ({
  path: relative(src, path),
  imports: imports(path),
}));

describe('architecture', () => {
  it('the dev proxy and the server agree on which paths are the API', () => {
    const rust = readFileSync(
      join(src, '../../server/src/routes/dashboard.rs'),
      'utf8',
    );
    const list = rust.match(/API_PREFIXES: \[&str; \d+\] = \[([^\]]+)\]/);
    expect(list).not.toBeNull();
    const server = [...(list?.[1] ?? '').matchAll(/"([^"]+)"/g)].map(
      (m) => m[1],
    );
    expect([...API_PREFIXES].sort()).toEqual(server.sort());
  });

  it('layers import downward only: routes → features → shared', () => {
    const offenders = all.flatMap(({ path, imports }) =>
      imports
        .filter(
          (target) =>
            (path.startsWith('shared/') &&
              /^@\/(features|routes)\//.test(target)) ||
            (path.startsWith('features/') && target.startsWith('@/routes/')),
        )
        .map((target) => `${path} → ${target}`),
    );
    expect(offenders).toEqual([]);
  });

  it('features do not import each other', () => {
    const offenders = all.flatMap(({ path, imports }) => {
      const own = path.match(/^features\/([^/]+)\//)?.[1];
      if (!own) return [];
      return imports
        .filter((target) => {
          const other = target.match(/^@\/features\/([^/]+)\//)?.[1];
          return other !== undefined && other !== own;
        })
        .map((target) => `${path} → ${target}`);
    });
    expect(offenders).toEqual([]);
  });

  it('there are no barrel files', () => {
    const barrels = all
      .map(({ path }) => path)
      .filter((path) => /(^|\/)index\.ts$/.test(path));
    expect(barrels).toEqual([]);
  });
});
