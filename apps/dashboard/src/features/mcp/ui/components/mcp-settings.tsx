import { Link, useRouter } from '@tanstack/react-router';
import { TriangleAlert } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'use-intl';
import type { McpSettings } from '@/shared/api/schemas';
import { readableMessage } from '@/shared/lib/form-errors';
import { CodeBlock, CopyButton } from '@/shared/ui/components/copy-button';
import { Switch } from '@/shared/ui/components/shadcn/switch';
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/shared/ui/components/shadcn/tabs';
import { updateMcpSettings } from '../../api/mutations';
import { mcpSnippet, type SnippetClient } from '../../model/snippets';

/**
 * The on/off of `/mcp`. Admins flip it; everyone else sees which way it is.
 * The switch moves at once; the server's answer settles it either way.
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
    <div className="space-y-2">
      <div className="flex items-center gap-3 text-sm">
        <Switch
          checked={enabled}
          onCheckedChange={toggle}
          disabled={!isAdmin || next !== null}
          aria-label={t('mcp.server.toggleLabel')}
        />
        <span>{enabled ? t('mcp.server.on') : t('mcp.server.off')}</span>
      </div>
      {!isAdmin && (
        <p className="text-xs text-muted-foreground">
          {t('mcp.server.adminOnly')}
        </p>
      )}
    </div>
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
            className="font-medium text-foreground underline underline-offset-4"
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
    <div className="space-y-6">
      <div className="space-y-1.5">
        <p className="text-sm font-medium">{t('mcp.server.endpoint')}</p>
        <div className="flex items-center gap-2 rounded-lg border bg-background py-1 ps-3 pe-1">
          <code className="min-w-0 flex-1 truncate font-mono text-xs">
            {url}
          </code>
          <CopyButton text={url} />
        </div>
      </div>
      <Tabs defaultValue="web">
        <TabsList>
          {(['web', ...CLIENTS] as const).map((client) => (
            <TabsTrigger key={client} value={client}>
              {clientName[client]}
            </TabsTrigger>
          ))}
        </TabsList>
        <TabsContent value="web">
          <p className="text-sm text-muted-foreground">
            {t('mcp.clients.webSteps')}
          </p>
        </TabsContent>
        {CLIENTS.map((client) => (
          <TabsContent key={client} value={client}>
            <CodeBlock code={mcpSnippet(client, url)} />
          </TabsContent>
        ))}
      </Tabs>
      <p className="text-xs text-muted-foreground">{t('mcp.page.note')}</p>
    </div>
  );
}

function Notice({ children }: { children: ReactNode }) {
  return (
    <div className="flex gap-3 rounded-xl border border-dashed p-5 text-sm text-muted-foreground">
      <TriangleAlert className="size-4 shrink-0" />
      <p>{children}</p>
    </div>
  );
}
