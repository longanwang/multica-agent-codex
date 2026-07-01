import type { DashboardSnapshot, TaskSummary } from './types';

const now = new Date().toISOString();

export const mockDashboard: DashboardSnapshot = {
  agents: [
    {
      id: 'fixture-fast-reviewer',
      name: '模拟快速审阅器',
      version: '0.1.0',
      source: 'fixture',
      acp: 'supported',
      runtime: 'fixture',
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
      runtime: 'cli',
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
      runtime: 'cli',
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
          label: '模拟小模型',
          sourceAgent: 'fixture-fast-reviewer',
          provider: null,
        },
        {
          id: 'fixture-large',
          label: '模拟大模型',
          sourceAgent: 'fixture-fast-reviewer',
          provider: null,
        },
      ],
      skills: [
        {
          id: 'fixture-fast-reviewer:fixture-review',
          name: '模拟审阅',
          sourceAgent: 'fixture-fast-reviewer',
          description: '用于验证 Multica 能力扫描流程的确定性测试技能。',
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
          description: '运行一次确定性的模拟审阅',
        },
      ],
      configOptions: [
        {
          id: 'fixture-small',
          label: '模拟小模型',
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
        title: '架构审阅',
        prompt: '审阅编排器设计并总结主要风险。',
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
            '模拟快速审阅器已完成任务：审阅编排器设计并总结主要风险。',
          startedAt: now,
          completedAt: now,
        },
        {
          id: 'run-demo-2',
          taskId: 'task-demo',
          agentId: 'claude-code',
          status: 'failed',
          error: '浏览器预览模式无法调用真实智能体命令。',
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
      summary: '飞书用户 user_42 请求启动智能体任务',
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
          ? `浏览器预览模拟运行完成：${prompt.slice(0, 120)}`
          : null,
      error:
        agentId === 'fixture-fast-reviewer'
          ? null
          : '真实 ACP 智能体执行需要在 Tauri 桌面模式中使用。',
      startedAt: createdAt,
      completedAt: createdAt,
    })),
  };
}
