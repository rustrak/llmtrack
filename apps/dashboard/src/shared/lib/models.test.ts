import { describe, expect, it } from 'vitest';
import { byName, isPicked, toggle } from './models';

const models = [
  { id: 1, name: 'gpt' },
  { id: 2, name: 'claude' },
  { id: 3, name: 'gpt' },
];

describe('models by name', () => {
  it('list each name once, by its first deployment', () => {
    expect(byName(models)).toEqual([
      { id: 1, name: 'gpt' },
      { id: 2, name: 'claude' },
    ]);
  });

  it('count a name as picked whichever of its deployments is', () => {
    expect(isPicked(models, [3], 'gpt')).toBe(true);
    expect(isPicked(models, [2], 'gpt')).toBe(false);
  });

  it('pick a name by its first deployment and drop all of it', () => {
    expect(toggle(models, [2], 'gpt', true)).toEqual([2, 1]);
    expect(toggle(models, [2, 3], 'gpt', false)).toEqual([2]);
  });
});
