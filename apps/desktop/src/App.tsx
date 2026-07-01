import {
  Activity,
  Bot,
  Check,
  ChevronRight,
  Clock3,
  Cpu,
  GitBranch,
  KeyRound,
  Layers3,
  MessageSquareText,
  Network,
  Play,
  RefreshCw,
  Search,
  Settings,
  ShieldAlert,
  SlidersHorizontal,
  TerminalSquare,
  X,
  Zap,
} from 'lucide-react';
import { FormEvent, useEffect, useMemo, useState } from 'react';
import { api } from './api';
import type {
  AgentProfile,
  CapabilityInventory,
  DashboardSnapshot,
  PermissionRequest,
  RunStatus,
  TaskSummary,
} from './types';

const navItems = [
  { id: 'agents', label: 'Agents', icon: Bot },
  { id: 'tasks', label: 'Tasks', icon: Play },
  { id: 'sessions', label: 'Sessions', icon: GitBranch },
  { id: 'capabilities', label: 'Capabilities', icon: Layers3 },
  { id: 'approvals', label: 'Approvals', icon: ShieldAlert },
  { id: 'connectors', label: 'Connectors', icon: MessageSquareText },
  { id: 'settings', label: 'Settings', icon: Settings },
] as const;

type NavId = (typeof navItems)[number]['id'];

export function App() {
  const [active, setActive] = useState<NavId>('agents');
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [selectedAgents, setSelectedAgents] = useState<string[]>(['fixture-fast-reviewer']);
  const [prompt, setPrompt] = useState('Compare the current repo architecture against the MVP plan and return risks.');
  const [title, setTitle] = useState('Parallel Architecture Review');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    void loadSnapshot();
  }, []);

  async function loadSnapshot() {
    setBusy(true);
    try {
      const data = await api.dashboardSnapshot();
      setSnapshot(data);
      if (selectedAgents.length === 0 && data.agents[0]) {
        setSelectedAgents([data.agents[0].id]);
      }
    } finally {
      setBusy(false);
    }
  }

  async function refreshCapabilities() {
    setBusy(true);
    try {
      const capabilities = await api.refreshCapabilities();
      setSnapshot((current) =>
        current
          ? {
              ...current,
              capabilities,
            }
          : current,
      );
      setNotice('Capability inventory refreshed');
    } finally {
      setBusy(false);
    }
  }

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!snapshot || selectedAgents.length === 0 || prompt.trim().length === 0) {
      return;
    }
    setBusy(true);
    try {
      const task = await api.createTask(title.trim() || 'Untitled Task', prompt.trim(), selectedAgents);
      setSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: [task, ...current.tasks.filter((item) => item.task.id !== task.task.id)],
            }
          : current,
      );
      setActive('tasks');
      setNotice('Task completed through the orchestrator');
    } finally {
      setBusy(false);
    }
  }

  async function decidePermission(request: PermissionRequest, approved: boolean) {
    setBusy(true);
    try {
      await api.decidePermission(request.id, approved);
      setSnapshot((current) =>
        current
          ? {
              ...current,
              approvals: current.approvals.filter((approval) => approval.id !== request.id),
            }
          : current,
      );
      setNotice(approved ? 'Remote task approved locally' : 'Remote task denied');
    } finally {
      setBusy(false);
    }
  }

  const content = useMemo(() => {
    if (!snapshot) {
      return <LoadingPanel />;
    }

    return (
      <>
        <OverviewStrip snapshot={snapshot} />
        <section className="workspace-grid">
          <AgentRoster
            agents={snapshot.agents}
            selectedAgents={selectedAgents}
            onToggle={(id) =>
              setSelectedAgents((current) =>
                current.includes(id) ? current.filter((item) => item !== id) : [...current, id],
              )
            }
          />
          <TaskComposer
            title={title}
            prompt={prompt}
            selectedAgents={selectedAgents}
            busy={busy}
            onTitleChange={setTitle}
            onPromptChange={setPrompt}
            onSubmit={submitTask}
          />
          <ApprovalQueue approvals={snapshot.approvals} onDecide={decidePermission} />
          <CapabilityPanel capabilities={snapshot.capabilities} agents={snapshot.agents} onRefresh={refreshCapabilities} />
        </section>
        <WorkspaceDetail active={active} snapshot={snapshot} />
      </>
    );
  }, [active, busy, prompt, selectedAgents, snapshot, title]);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">M</div>
          <div>
            <strong>Multica</strong>
            <span>ACP control shell</span>
          </div>
        </div>
        <nav className="nav-list">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                className={active === item.id ? 'nav-item is-active' : 'nav-item'}
                type="button"
                onClick={() => setActive(item.id)}
              >
                <Icon size={18} aria-hidden="true" />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
        <div className="sidebar-status">
          <div className="pulse" />
          <span>Local-first mode</span>
        </div>
      </aside>

      <main className="main-surface">
        <header className="topbar">
          <div>
            <h1>Agent Operations</h1>
            <p>Manage ACP agents, capability inventory, approvals, and IM ingress from one desktop console.</p>
          </div>
          <div className="topbar-actions">
            <button className="icon-button" type="button" title="Refresh dashboard" onClick={loadSnapshot} disabled={busy}>
              <RefreshCw size={18} aria-hidden="true" />
            </button>
            <button className="primary-button" type="button" onClick={() => setActive('tasks')}>
              <Play size={17} aria-hidden="true" />
              New task
            </button>
          </div>
        </header>

        {notice ? (
          <div className="notice" role="status">
            <Check size={16} aria-hidden="true" />
            <span>{notice}</span>
            <button type="button" onClick={() => setNotice(null)} title="Dismiss">
              <X size={14} aria-hidden="true" />
            </button>
          </div>
        ) : null}

        {content}
      </main>
    </div>
  );
}

function OverviewStrip({ snapshot }: { snapshot: DashboardSnapshot }) {
  const modelCount = snapshot.capabilities.reduce((sum, item) => sum + item.models.length, 0);
  const skillCount = snapshot.capabilities.reduce((sum, item) => sum + item.skills.length, 0);
  const mcpCount = snapshot.capabilities.reduce((sum, item) => sum + item.mcpServers.length, 0);
  const running = snapshot.tasks.filter((task) => task.status === 'running').length;
  return (
    <section className="metric-strip" aria-label="System overview">
      <Metric icon={Bot} label="Agents" value={snapshot.agents.length.toString()} tone="teal" />
      <Metric icon={Cpu} label="Models" value={modelCount.toString()} tone="indigo" />
      <Metric icon={Zap} label="Skills" value={skillCount.toString()} tone="amber" />
      <Metric icon={Network} label="MCP Servers" value={mcpCount.toString()} tone="teal" />
      <Metric icon={Activity} label="Running" value={running.toString()} tone="indigo" />
    </section>
  );
}

function Metric({
  icon: Icon,
  label,
  value,
  tone,
}: {
  icon: typeof Bot;
  label: string;
  value: string;
  tone: 'teal' | 'amber' | 'indigo';
}) {
  return (
    <div className={`metric metric-${tone}`}>
      <Icon size={18} aria-hidden="true" />
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function AgentRoster({
  agents,
  selectedAgents,
  onToggle,
}: {
  agents: AgentProfile[];
  selectedAgents: string[];
  onToggle: (id: string) => void;
}) {
  return (
    <section className="panel span-2">
      <PanelHeader icon={Bot} title="Agent Roster" action="Select for parallel runs" />
      <div className="agent-table">
        {agents.map((agent) => (
          <button
            key={agent.id}
            type="button"
            className={selectedAgents.includes(agent.id) ? 'agent-row selected' : 'agent-row'}
            onClick={() => onToggle(agent.id)}
          >
            <span className="check-slot">{selectedAgents.includes(agent.id) ? <Check size={14} /> : null}</span>
            <span>
              <strong>{agent.name}</strong>
              <small>{agent.launch.command}</small>
            </span>
            <StatusPill status={agent.acp === 'supported' ? 'succeeded' : 'queued'} label={agent.acp === 'supported' ? 'ACP' : 'Probe'} />
            <span className="source-chip">{agent.source}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function TaskComposer({
  title,
  prompt,
  selectedAgents,
  busy,
  onTitleChange,
  onPromptChange,
  onSubmit,
}: {
  title: string;
  prompt: string;
  selectedAgents: string[];
  busy: boolean;
  onTitleChange: (value: string) => void;
  onPromptChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <section className="panel">
      <PanelHeader icon={TerminalSquare} title="Parallel Task" action={`${selectedAgents.length} selected`} />
      <form className="task-form" onSubmit={onSubmit}>
        <label>
          <span>Title</span>
          <input value={title} onChange={(event) => onTitleChange(event.target.value)} />
        </label>
        <label>
          <span>Prompt</span>
          <textarea value={prompt} onChange={(event) => onPromptChange(event.target.value)} rows={5} />
        </label>
        <button className="primary-button full" type="submit" disabled={busy || selectedAgents.length === 0}>
          <Play size={17} aria-hidden="true" />
          Run agents
        </button>
      </form>
    </section>
  );
}

function ApprovalQueue({
  approvals,
  onDecide,
}: {
  approvals: PermissionRequest[];
  onDecide: (request: PermissionRequest, approved: boolean) => void;
}) {
  return (
    <section className="panel">
      <PanelHeader icon={ShieldAlert} title="Approvals" action={`${approvals.length} pending`} />
      <div className="approval-list">
        {approvals.length === 0 ? <EmptyState label="No remote requests waiting" /> : null}
        {approvals.map((approval) => (
          <article className="approval-card" key={approval.id}>
            <div>
              <strong>{approval.summary}</strong>
              <p>{String(approval.details.preview ?? 'Remote connector request')}</p>
            </div>
            <div className="approval-actions">
              <button type="button" className="icon-button approve" title="Approve" onClick={() => onDecide(approval, true)}>
                <Check size={16} aria-hidden="true" />
              </button>
              <button type="button" className="icon-button deny" title="Deny" onClick={() => onDecide(approval, false)}>
                <X size={16} aria-hidden="true" />
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function CapabilityPanel({
  capabilities,
  agents,
  onRefresh,
}: {
  capabilities: CapabilityInventory[];
  agents: AgentProfile[];
  onRefresh: () => void;
}) {
  const agentName = (agentId: string) => agents.find((agent) => agent.id === agentId)?.name ?? agentId;
  return (
    <section className="panel span-2">
      <PanelHeader icon={Layers3} title="Capability Inventory" action="Models, skills, MCP" />
      <button className="secondary-button inventory-refresh" type="button" onClick={onRefresh}>
        <RefreshCw size={16} aria-hidden="true" />
        Refresh inventory
      </button>
      <div className="capability-grid">
        {capabilities.map((inventory) => (
          <article key={inventory.agentId} className="capability-card">
            <header>
              <strong>{agentName(inventory.agentId)}</strong>
              <small>{inventory.models.length} models</small>
            </header>
            <CapabilityLine label="Skills" value={inventory.skills.map((skill) => skill.name).join(', ') || 'None found'} />
            <CapabilityLine
              label="MCP"
              value={inventory.mcpServers.map((server) => server.name).join(', ') || 'None found'}
            />
            <CapabilityLine
              label="Commands"
              value={inventory.slashCommands.map((command) => command.name).join(', ') || 'None found'}
            />
          </article>
        ))}
      </div>
    </section>
  );
}

function CapabilityLine({ label, value }: { label: string; value: string }) {
  return (
    <p className="capability-line">
      <span>{label}</span>
      <strong>{value}</strong>
    </p>
  );
}

function WorkspaceDetail({ active, snapshot }: { active: NavId; snapshot: DashboardSnapshot }) {
  if (active === 'tasks' || active === 'sessions') {
    return <TaskTimeline tasks={snapshot.tasks} />;
  }
  if (active === 'connectors') {
    return <ConnectorPanel />;
  }
  if (active === 'settings') {
    return <SettingsPanel />;
  }
  return null;
}

function TaskTimeline({ tasks }: { tasks: TaskSummary[] }) {
  return (
    <section className="wide-panel">
      <PanelHeader icon={Clock3} title="Task Timeline" action={`${tasks.length} recent`} />
      <div className="timeline">
        {tasks.map((task) => (
          <article className="task-row" key={task.task.id}>
            <div className="task-main">
              <StatusPill status={task.status} label={task.status} />
              <div>
                <strong>{task.task.title}</strong>
                <p>{task.task.prompt}</p>
              </div>
            </div>
            <div className="run-list">
              {task.runs.map((run) => (
                <div className="run-item" key={run.id}>
                  <span>{run.agentId}</span>
                  <StatusPill status={run.status} label={run.status} />
                  <p>{run.result || run.error}</p>
                </div>
              ))}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function ConnectorPanel() {
  return (
    <section className="wide-panel connector-board">
      <PanelHeader icon={MessageSquareText} title="Connectors" action="Relay routes" />
      <ConnectorCard name="Feishu Bot" status="Ready first" accent="teal" />
      <ConnectorCard name="WeCom App" status="Next adapter" accent="amber" />
      <ConnectorCard name="Local Simulator" status="Developer ingress" accent="indigo" />
    </section>
  );
}

function ConnectorCard({ name, status, accent }: { name: string; status: string; accent: string }) {
  return (
    <article className={`connector-card ${accent}`}>
      <MessageSquareText size={20} aria-hidden="true" />
      <div>
        <strong>{name}</strong>
        <span>{status}</span>
      </div>
      <ChevronRight size={16} aria-hidden="true" />
    </article>
  );
}

function SettingsPanel() {
  return (
    <section className="wide-panel settings-board">
      <PanelHeader icon={SlidersHorizontal} title="Settings" action="Local control plane" />
      <div className="settings-row">
        <KeyRound size={18} aria-hidden="true" />
        <div>
          <strong>Remote requests require local approval</strong>
          <span>Connector-triggered tasks enter the desktop approval queue before any agent run.</span>
        </div>
      </div>
      <div className="settings-row">
        <Search size={18} aria-hidden="true" />
        <div>
          <strong>Agent discovery is non-installing</strong>
          <span>PATH, common Windows locations, ACP registry hints, and manual profiles are supported.</span>
        </div>
      </div>
    </section>
  );
}

function PanelHeader({
  icon: Icon,
  title,
  action,
}: {
  icon: typeof Bot;
  title: string;
  action: string;
}) {
  return (
    <header className="panel-header">
      <div>
        <Icon size={18} aria-hidden="true" />
        <h2>{title}</h2>
      </div>
      <span>{action}</span>
    </header>
  );
}

function StatusPill({ status, label }: { status: RunStatus | 'queued'; label: string }) {
  return <span className={`status-pill ${status}`}>{label}</span>;
}

function EmptyState({ label }: { label: string }) {
  return <div className="empty-state">{label}</div>;
}

function LoadingPanel() {
  return (
    <section className="wide-panel loading-panel">
      <RefreshCw size={22} aria-hidden="true" />
      <span>Loading local control plane</span>
    </section>
  );
}

