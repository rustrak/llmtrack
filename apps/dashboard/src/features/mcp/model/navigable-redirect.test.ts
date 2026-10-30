import { describe, expect, it } from 'vitest';
import { navigableRedirect } from './navigable-redirect';

describe('navigableRedirect', () => {
  it('lets the browser go to a web page or an app', () => {
    for (const url of [
      'https://claude.ai/api/mcp/auth_callback?code=x',
      'http://localhost:49581/callback?code=x',
      'cursor://anysphere.cursor-mcp/oauth/callback?code=x',
    ]) {
      expect(navigableRedirect(url)).toBe(true);
    }
  });

  it('never to something that runs in this page', () => {
    for (const url of [
      'javascript:alert(document.domain)//?code=x',
      'JavaScript:alert(1)',
      'data:text/html,<script>alert(1)</script>',
      'vbscript:msgbox(1)',
      'not a url',
    ]) {
      expect(navigableRedirect(url)).toBe(false);
    }
  });
});
