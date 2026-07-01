import type { DashboardSnapshot, TaskSummary } from './types';

const now = new Date().toISOString();

export const mockDashboard: DashboardSnapshot = {
  agents: [
    {
      id: 'fixture-fast-reviewer',
      name: 'Fixture Fast Reviewer',
      version: '0.1.0',
      source: 'fixture',
      acp: 'supported',
      detectedPath: null,
      lastSeen: now,
      launch: {
        command: 'multica-fake',
        args: ['success'],
        env: {},
        cwd: null,
      },
    },
    {
      id: 'claude-code',
      name: 'Claude Code',
      version: null,
      source: 'path',
      acp: 'unknown',
      detectedPath: 'C:\\Users\\you\\AppData\\Roaming\\npm\\claude.cmd',
      lastSeen: now,
      launch: {
        command: 'claude',
        args: [],
        env: {},
        cwd: null,
      },
    },
    {
      id: 'gemini',
      name: 'Gemini CLI',
      version: null,
      source: 'path',
      acp: 'unknown',
      detectedPath: 'C:\\Users\\you\\AppData\\Roaming\\npm\\gemini.cmd',
      lastSeen: now,
      launch: {
        command: 'gemini',
        args: [],
        env: {},
        cwd: null,
      },
    },
  ],
  capabilities: [
    {
      agentId: 'fixture-fast-reviewer',
      auth: 'ready',
      scannedAt: now,
      models: [
        {
          id: 'fixture-small',
          label: 'Fixture Small',
          sourceAgent: 'fixture-fast-reviewer',
          provider: null,
        },
        {
          id: 'fixture-large',
          label: 'Fixture Large',
          sourceAgent: 'fixture-fast-reviewer',
          provider: null,
        },
      ],
      skills: [
        {
          id: 'fixture-fast-reviewer:fixture-review',
          name: 'fixture-review',
          sourceAgent: 'fixture-fast-reviewer',
          description: 'Deterministic test skill used by Multica capability scanner tests.',
          sourcePath: 'crates/core/tests/fixtures/skills/review/SKILL.md',
        },
      ],
      mcpServers: [
        {
          id: 'fixture-fast-reviewer:fixture-search',
          name: 'fixture-search',
          sourceAgent: 'fixture-fast-reviewer',
          command: 'node',
          enabled: true,
          sourcePath: 'crates/core/tests/fixtures/mcp/fixture.json',
        },
      ],
      slashCommands: [
        {
          name: '/review',
          sourceAgent: 'fixture-fast-reviewer',
          description: 'Run a deterministic fixture review',
        },
      ],
      configOptions: [
        {
          id: 'fixture-small',
          label: 'Fixture Small',
          category: 'model',
          valueType: 'select',
          choices: ['fixture-small', 'fixture-large'],
          raw: {},
        },
      ],
    },
  ],
  tasks: [
    {
      status: 'succeeded',
      completedAt: now,
      task: {
        id: 'task-demo',
        title: 'Architecture Review',
        prompt: 'Review the orchestrator design and summarize risks.',
        agentIds: ['fixture-fast-reviewer', 'claude-code'],
        requestedBy: 'localUser',
        createdAt: now,
      },
      runs: [
        {
          id: 'run-demo-1',
          taskId: 'task-demo',
          agentId: 'fixture-fast-reviewer',
          status: 'succeeded',
          result:
            'Fixture Fast Reviewer completed fixture run for prompt: Review the orchestrator design and summarize risks.',
          startedAt: now,
          completedAt: now,
        },
        {
          id: 'run-demo-2',
          taskId: 'task-demo',
          agentId: 'claude-code',
          status: 'failed',
          error: 'Agent command not available in browser preview.',
          startedAt: now,
          completedAt: now,
        },
      ],
    },
  ],
  approvals: [
    {
      id: 'approval-demo',
      taskId: null,
      connectorMessageId: 'feishu-msg-001',
      kind: 'connectorReply',
      summary: 'Feishu message from user_42 wants to start an agent task',
      details: {
        connector: 'feishu',
        tenantId: 'tenant_demo',
        conversationId: 'chat_demo',
        senderId: 'user_42',
        preview: '请让 Claude Code 和 Gemini 并行检查这个 PR。',
      },
      status: 'pending',
      createdAt: now,
    },
  ],
};

export function makeMockTask(title: string, prompt: string, agentIds: string[]): TaskSummary {
  const createdAt = new Date().toISOString();
  return {
    status: 'succeeded',
    completedAt: createdAt,
    task: {
      id: crypto.randomUUID(),
      title,
      prompt,
      agentIds,
      requestedBy: 'localUser',
      createdAt,
    },
    runs: agentIds.map((agentId) => ({
      id: crypto.randomUUID(),
      taskId: 'browser-preview',
      agentId,
      status: agentId === 'fixture-fast-reviewer' ? 'succeeded' : 'failed',
      result:
        agentId === 'fixture-fast-reviewer'
          ? `Browser preview fixture completed: ${prompt.slice(0, 120)}`
          : null,
      error:
        agentId === 'fixture-fast-reviewer'
          ? null
          : 'Real ACP agent execution is available in Tauri desktop mode.',
      startedAt: createdAt,
      completedAt: createdAt,
    })),
  };
}

