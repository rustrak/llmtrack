import {
  Switch,
  Tab,
  TabList,
  TabPanel,
  Tabs,
  Tag,
  WarningIcon,
} from '@rustrak/ui';
import { Link, useRouter } from '@tanstack/react-router';
import { type ReactNode, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { McpSettings } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { CodeBlock, CopyButton } from '@/shared/ui/components/copy-button';
import { SettingRow } from '@/shared/ui/components/setting-row';
import { updateMcpSettings } from '../../api/mutations';
import { mcpSnippet, type SnippetClient } from '../../model/snippets';

/**
 * The on/off of `/mcp`, as a settings row. Admins flip it; everyone else sees
 * which way it is. The switch moves at once; the server's answer settles it
 * either way.
 */
export function McpServerToggle({
  settings,
  isAdmin,
}: {
  settings: McpSettings;
  isAdmin: boolean;
}) {
  const t = useTranslations('settings');
  const router = useRouter();
  const [next, setNext] = useState<boolean | null>(null);
  const enabled = next ?? settings.enabled;

  const toggle = async (value: boolean) => {
    setNext(value);
    const result = await updateMcpSettings(value);
    if (!result.success) {
      toast.error(t('mcp.server.updateFailed'), {
        description: readableMessage(result.error),
      });
    }
    await router.invalidate();
    setNext(null);
  };

  return (
    <SettingRow
      title={t('mcp.server.title')}
      description={
        isAdmin ? t('mcp.server.description') : t('mcp.server.adminOnly')
      }
    >
      <div className="flex items-center gap-3">
        <Switch
          checked={enabled}
          onCheckedChange={toggle}
          disabled={!isAdmin || next !== null}
          aria-label={t('mcp.server.toggleLabel')}
        />
        <Tag tone={enabled ? 'brand' : 'neutral'}>
          {enabled ? t('mcp.server.on') : t('mcp.server.off')}
        </Tag>
      </div>
    </SettingRow>
  );
}

const CLIENTS: readonly SnippetClient[] = ['claudeCode', 'json', 'vscode'];

/**
 * How to connect an assistant, while the server is on: the URL, then what to
 * paste into the client you use. Signing in is the only way in, and it needs
 * the Public URL, so without one the page says where to set it.
 */
export function McpConnect({
  settings,
  isAdmin,
}: {
  settings: McpSettings;
  isAdmin: boolean;
}) {
  const t = useTranslations('settings');
  const clientName: Record<SnippetClient | 'web', string> = {
    web: t('mcp.clients.web'),
    claudeCode: t('mcp.clients.claudeCode'),
    json: t('mcp.clients.json'),
    vscode: t('mcp.clients.vscode'),
  };

  if (!settings.enabled) {
    return (
      <Notice>
        {isAdmin ? t('mcp.page.offAdmin') : t('mcp.page.offMember')}
      </Notice>
    );
  }

  const { url } = settings;
  if (url === null) {
    return (
      <Notice>
        {t('mcp.page.needsPublicUrl')}{' '}
        {isAdmin ? (
          <Link
            to="/settings/general"
            className="font-medium text-fg-brand hover:underline"
          >
            {t('mcp.page.setPublicUrl')}
          </Link>
        ) : (
          t('mcp.page.askAdmin')
        )}
      </Notice>
    );
  }

  return (
    <>
      <SettingRow title={t('mcp.server.endpoint')} wide>
        <div className="flex items-center gap-2 rounded-md border border-border-field bg-canvas py-1 ps-3 pe-1">
          <code className="min-w-0 flex-1 truncate font-mono text-mono text-fg">
            {url}
          </code>
          <CopyButton text={url} />
        </div>
      </SettingRow>
      <div className="px-5 py-4">
        <Tabs defaultValue="web">
          <TabList>
            {(['web', ...CLIENTS] as const).map((client) => (
              <Tab key={client} value={client} size="sm">
                {clientName[client]}
              </Tab>
            ))}
          </TabList>
          <TabPanel value="web">
            <p className="text-body text-fg-subtle">
              {t('mcp.clients.webSteps')}
            </p>
          </TabPanel>
          {CLIENTS.map((client) => (
            <TabPanel key={client} value={client}>
              <CodeBlock code={mcpSnippet(client, url)} />
            </TabPanel>
          ))}
        </Tabs>
        <p className="mt-4 text-hint text-fg-subtle">{t('mcp.page.note')}</p>
      </div>
    </>
  );
}

function Notice({ children }: { children: ReactNode }) {
  return (
    <div className="flex gap-3 px-5 py-4 text-body text-fg-subtle">
      <WarningIcon size="md" className="mt-1 shrink-0 text-sev-warning-fg" />
      <p>{children}</p>
    </div>
  );
}
