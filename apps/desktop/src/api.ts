import { makeMockTask, mockDashboard } from './mockData';
import type { CapabilityInventory, DashboardSnapshot, TaskSummary } from './types';

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
    const payload = args as { title: string; prompt: string; agent_ids: string[] };
    return makeMockTask(payload.title, payload.prompt, payload.agent_ids) as never;
  }
  if (command === 'decide_permission') {
    return undefined as never;
  }
  throw new Error(`Unknown mock command: ${command}`);
};

export const api = {
  dashboardSnapshot: () => invoke<DashboardSnapshot>('dashboard_snapshot'),
  refreshAgents: () => invoke<DashboardSnapshot['agents']>('refresh_agents'),
  refreshCapabilities: () => invoke<CapabilityInventory[]>('refresh_capabilities'),
  createTask: (title: string, prompt: string, agentIds: string[]) =>
    invoke<TaskSummary>('create_task', { title, prompt, agent_ids: agentIds }),
  decidePermission: (requestId: string, approved: boolean) =>
    invoke<void>('decide_permission', { request_id: requestId, approved }),
};
