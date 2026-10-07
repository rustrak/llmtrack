/** Where an invitee sets their password: a public page of this dashboard. */
export function inviteLink(origin: string, token: string) {
  return `${origin}/invite/${token}`;
}

/** Still usable: pending and not past its expiry. */
export function isOpen(
  invite: { status: string; expires_at: string },
  now = new Date(),
) {
  return (
    invite.status === 'pending' && Date.parse(invite.expires_at) > now.getTime()
  );
}
