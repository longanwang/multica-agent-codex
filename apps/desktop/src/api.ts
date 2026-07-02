import { makeMockTask, mockDashboard } from './mockData';
import type { AgentProfile, CapabilityInventory, DashboardSnapshot, PermissionRequest, TaskSummary } from './types';

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    const api = await import('@tauri-apps/api/core');
    return api.invoke<T>(command, args);
  }

  return mockInvoke<T>(command, args);
}

const mockInvoke: Invoke = async (command, args) => {
  await new Promise((resolve) => window.setTimeout(resolve, 180));
  if (command === 'dashboard_snapshot') {
    return structuredClone(mockDashboard) as never;
  }
  if (command === 'refresh_agents') {
    return structuredClone(mockDashboard.agents) as never;
  }
  if (command === 'refresh_capabilities') {
    return structuredClone(mockDashboard.capabilities) as never;
  }
  if (command === 'create_task') {
    const payload = args as { title: string; prompt: string; agentIds: string[] };
    return makeMockTask(payload.title, payload.prompt, payload.agentIds) as never;
  }
  if (command === 'add_manual_agent') {
    const payload = args as { name: string; command: string; args: string[] };
    return {
      id: `manual-${slug(payload.name)}`,
      name: payload.name,
      version: null,
      launch: {
        command: payload.command,
        args: payload.args,
        env: {},
        cwd: null,
      },
      runtime: 'cli',
      acp: 'unknown',
      source: 'manual',
      detectedPath: null,
      lastSeen: new Date().toISOString(),
    } satisfies AgentProfile as never;
  }
  if (command === 'ingest_connector_message') {
    const payload = args as {
      connector: string;
      tenantId: string;
      conversationId: string;
      senderId: string;
      text: string;
      raw: unknown;
    };
    return {
      id: crypto.randomUUID(),
      taskId: null,
      connectorMessageId: crypto.randomUUID(),
      kind: 'connectorReply',
      summary: `${payload.connector} 用户 ${payload.senderId} 请求启动智能体任务`,
      details: {
        connector: payload.connector,
        tenantId: payload.tenantId,
        conversationId: payload.conversationId,
        senderId: payload.senderId,
        preview: payload.text.slice(0, 240),
      },
      status: 'pending',
      createdAt: new Date().toISOString(),
    } satisfies PermissionRequest as never;
  }
  if (command === 'decide_permission') {
    return undefined as never;
  }
  throw new Error(`Unknown mock command: ${command}`);
};

export const api = {
  isDesktopRuntime: isTauriRuntime,
  dashboardSnapshot: () => invoke<DashboardSnapshot>('dashboard_snapshot'),
  refreshAgents: () => invoke<DashboardSnapshot['agents']>('refresh_agents'),
  refreshCapabilities: () => invoke<CapabilityInventory[]>('refresh_capabilities'),
  createTask: (title: string, prompt: string, agentIds: string[]) =>
    invoke<TaskSummary>('create_task', { title, prompt, agentIds }),
  addManualAgent: (name: string, command: string, args: string[]) =>
    invoke<AgentProfile>('add_manual_agent', { name, command, args }),
  ingestConnectorMessage: (message: {
    connector: string;
    tenantId: string;
    conversationId: string;
    senderId: string;
    text: string;
  }) =>
    invoke<PermissionRequest>('ingest_connector_message', {
      connector: message.connector,
      tenantId: message.tenantId,
      conversationId: message.conversationId,
      senderId: message.senderId,
      text: message.text,
      raw: message,
    }),
  decidePermission: (requestId: string, approved: boolean) =>
    invoke<void>('decide_permission', { requestId, approved }),
};

function slug(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, '-')
    .replace(/(^-|-$)/g, '');
}
