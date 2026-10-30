import { createFileRoute } from '@tanstack/react-router';
import { useTranslations } from 'use-intl';
import { getMcpSettings, listGrants } from '@/features/mcp/api/queries';
import { ConnectedApps } from '@/features/mcp/ui/components/connected-apps';
import {
  McpConnect,
  McpServerToggle,
} from '@/features/mcp/ui/components/mcp-settings';
import { unwrap } from '@/shared/api/http';
import { translator } from '@/shared/i18n/intl';
import { useCurrentUser } from '@/shared/ui/hooks/use-current-user';
import { Section, SettingsHeader } from './-components/section';

export const Route = createFileRoute('/_authenticated/settings/mcp')({
  loader: async () => {
    const [settings, grants] = await Promise.all([
      getMcpSettings(),
      listGrants(),
    ]);
    return { settings: unwrap(settings), grants: unwrap(grants) };
  },
  head: () => ({ meta: [{ title: translator('settings')('mcp.meta') }] }),
  component: McpPage,
});

function McpPage() {
  const t = useTranslations('settings');
  const { settings, grants } = Route.useLoaderData();
  const isAdmin = useCurrentUser().role === 'admin';
  return (
    <div>
      <SettingsHeader title={t('mcp.title')} subtitle={t('mcp.subtitle')} />
      <Section
        title={t('mcp.server.title')}
        description={t('mcp.server.description')}
      >
        <McpServerToggle settings={settings} isAdmin={isAdmin} />
      </Section>
      <Section
        title={t('mcp.page.connect')}
        description={t('mcp.page.connectDescription')}
      >
        <McpConnect settings={settings} isAdmin={isAdmin} />
      </Section>
      {/* With MCP off too: the grants are still stored, and withdrawing one
          must not wait for an admin to turn the server back on. */}
      <Section
        title={t('mcp.connected.title')}
        description={t('mcp.connected.description')}
      >
        <ConnectedApps grants={grants} />
      </Section>
    </div>
  );
}
