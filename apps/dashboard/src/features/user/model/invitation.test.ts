import { describe, expect, it } from 'vitest';
import { inviteLink, isOpen } from './invitation';

describe('invitations', () => {
  it('link to the public invite page of this dashboard', () => {
    expect(inviteLink('https://llm.acme.dev', 'abc')).toBe(
      'https://llm.acme.dev/invite/abc',
    );
  });

  it('are open while pending and unexpired', () => {
    const now = new Date('2026-10-07T12:00:00Z');
    const invite = { status: 'pending', expires_at: '2026-10-08T12:00:00Z' };
    expect(isOpen(invite, now)).toBe(true);
    expect(isOpen({ ...invite, status: 'accepted' }, now)).toBe(false);
    expect(isOpen({ ...invite, expires_at: '2026-10-07T11:59:59Z' }, now)).toBe(
      false,
    );
  });
});
