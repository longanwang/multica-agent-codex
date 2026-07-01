export type AcpSupport =
  | 'unknown'
  | 'supported'
  | { unavailable: { reason: string } };

export type AgentSource =
  | 'registry'
  | 'path'
  | 'commonLocation'
  | 'manual'
  | 'fixture';

export interface AgentLaunch {
  command: string;
  args: string[];
  env: Record<string, string>;
  cwd?: string | null;
}

export type AgentRuntime = 'acpStdio' | 'cli' | 'fixture';

export interface AgentProfile {
  id: string;
  name: string;
  version?: string | null;
  launch: AgentLaunch;
  runtime: AgentRuntime;
  acp: AcpSupport;
  source: AgentSource;
  detectedPath?: string | null;
  lastSeen: string;
}

export interface ModelOption {
  id: string;
  label: string;
  sourceAgent: string;
  provider?: string | null;
}

export interface SkillRef {
  id: string;
  name: string;
  sourceAgent: string;
  sourcePath?: string | null;
  description?: string | null;
}

export interface McpServerRef {
  id: string;
  name: string;
  sourceAgent: string;
  sourcePath?: string | null;
  command?: string | null;
  enabled: boolean;
}

export interface SlashCommandRef {
  name: string;
  sourceAgent: string;
  description?: string | null;
}

export interface ConfigOption {
  id: string;
  label: string;
  category?: string | null;
  valueType: string;
  choices: string[];
  raw: unknown;
}

export interface CapabilityInventory {
  agentId: string;
  models: ModelOption[];
  skills: SkillRef[];
  mcpServers: McpServerRef[];
  slashCommands: SlashCommandRef[];
  configOptions: ConfigOption[];
  auth: unknown;
  scannedAt: string;
}

export type RunStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'needsApproval';

export interface TaskSpec {
  id: string;
  title: string;
  prompt: string;
  agentIds: string[];
  requestedBy: unknown;
  createdAt: string;
}

export interface AgentRun {
  id: string;
  taskId: string;
  agentId: string;
  status: RunStatus;
  transcriptPath?: string | null;
  result?: string | null;
  error?: string | null;
  startedAt: string;
  completedAt?: string | null;
}

export interface TaskSummary {
  task: TaskSpec;
  status: RunStatus;
  runs: AgentRun[];
  completedAt?: string | null;
}

export interface PermissionRequest {
  id: string;
  taskId?: string | null;
  connectorMessageId?: string | null;
  kind: string;
  summary: string;
  details: Record<string, unknown>;
  status: string;
  createdAt: string;
}

export interface DashboardSnapshot {
  agents: AgentProfile[];
  capabilities: CapabilityInventory[];
  tasks: TaskSummary[];
  approvals: PermissionRequest[];
}
