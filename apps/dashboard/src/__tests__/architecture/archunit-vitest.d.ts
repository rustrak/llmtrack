import type { CheckOptions } from 'archunit';
import 'vitest';

// archunit 2.5 types `toPassAsync` through the `jest.Matchers` global that
// vitest 4 mirrored and vitest 5 dropped. Remove once archunit augments `vitest`.
declare module 'vitest' {
  interface Matchers<R, T> {
    toPassAsync: (options?: CheckOptions) => Promise<void>;
  }
}
