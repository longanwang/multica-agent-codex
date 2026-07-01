import {
  Activity,
  Bot,
  Check,
  Clock3,
  Cpu,
  GitBranch,
  KeyRound,
  Layers3,
  MessageSquareText,
  Network,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings,
  ShieldAlert,
  SlidersHorizontal,
  TerminalSquare,
  X,
  Zap,
} from 'lucide-react';
import { FormEvent, ReactNode, useEffect, useMemo, useState } from 'react';
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
  { id: 'agents', label: '智能体', icon: Bot },
  { id: 'tasks', label: '任务', icon: Play },
  { id: 'sessions', label: '会话', icon: GitBranch },
  { id: 'capabilities', label: '能力', icon: Layers3 },
  { id: 'approvals', label: '审批', icon: ShieldAlert },
  { id: 'connectors', label: '连接器', icon: MessageSquareText },
  { id: 'settings', label: '设置', icon: Settings },
] as const;

type NavId = (typeof navItems)[number]['id'];

const pageCopy: Record<NavId, { title: string; description: string }> = {
  agents: {
    title: '智能体资产',
    description: '发现本机 agent、补充手动启动配置，并确认运行方式。',
  },
  tasks: {
    title: '任务编排',
    description: '选择一个或多个 agent 并行运行，查看每个运行结果。',
  },
  sessions: {
    title: '会话与运行',
    description: '按任务追踪 agent run、错误、耗时和上下文传递结果。',
  },
  capabilities: {
    title: '能力清单',
    description: '汇总模型、skills、MCP 服务、命令和认证状态。',
  },
  approvals: {
    title: '审批中心',
    description: '处理远程连接器触发的任务和高风险动作。',
  },
  connectors: {
    title: '即时通讯连接器',
    description: '配置飞书、企业微信和本地模拟入口，把消息转成审批任务。',
  },
  settings: {
    title: '控制平面设置',
    description: '管理本地优先策略、发现路径、运行安全和 Relay 参数。',
  },
};

export function App() {
  const desktopRuntime = api.isDesktopRuntime();
  const [active, setActive] = useState<NavId>('agents');
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [prompt, setPrompt] = useState('请审阅当前仓库的核心风险，并给出可执行的修复建议。');
  const [title, setTitle] = useState('本地 agent 并行审阅');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    void loadSnapshot(true);
  }, []);

  async function loadSnapshot(autoDiscover = false) {
    setBusy(true);
    try {
      let data = await api.dashboardSnapshot();
      if (autoDiscover && data.agents.length === 0) {
        const agents = await api.refreshAgents();
        data = { ...data, agents };
      }
      setSnapshot(data);
      setSelectedAgents((current) => reconcileSelectedAgents(current, data.agents));
    } finally {
      setBusy(false);
    }
  }

  async function refreshAgents() {
    setBusy(true);
    try {
      const agents = await api.refreshAgents();
      setSnapshot((current) => (current ? { ...current, agents } : current));
      setSelectedAgents((current) => reconcileSelectedAgents(current, agents));
      setNotice('智能体发现已刷新');
    } finally {
      setBusy(false);
    }
  }

  async function refreshCapabilities() {
    setBusy(true);
    try {
      const capabilities = await api.refreshCapabilities();
      setSnapshot((current) => (current ? { ...current, capabilities } : current));
      setNotice('能力清单已刷新');
    } finally {
      setBusy(false);
    }
  }

  async function addManualAgent(name: string, command: string, args: string[]) {
    if (!name.trim() || !command.trim()) {
      setNotice('请填写智能体名称和启动命令');
      return;
    }
    setBusy(true);
    try {
      const agent = await api.addManualAgent(name.trim(), command.trim(), args);
      setSnapshot((current) =>
        current ? { ...current, agents: [agent, ...current.agents.filter((item) => item.id !== agent.id)] } : current,
      );
      setSelectedAgents((current) => (current.includes(agent.id) ? current : [...current, agent.id]));
      setNotice(`${agent.name} 已添加到智能体列表`);
    } finally {
      setBusy(false);
    }
  }

  async function submitTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!snapshot || selectedAgents.length === 0 || prompt.trim().length === 0) {
      return;
    }
    const selectedProfiles = snapshot.agents.filter((agent) => selectedAgents.includes(agent.id));
    if (!desktopRuntime && selectedProfiles.some((agent) => agent.source !== 'fixture')) {
      setNotice('Web 预览只运行模拟智能体；真实调用本地 agent 请用 Tauri 桌面模式启动。');
      return;
    }
    setBusy(true);
    try {
      const task = await api.createTask(title.trim() || '未命名任务', prompt.trim(), selectedAgents);
      setSnapshot((current) =>
        current ? { ...current, tasks: [task, ...current.tasks.filter((item) => item.task.id !== task.task.id)] } : current,
      );
      setActive('sessions');
      setNotice('任务已提交并完成运行');
    } finally {
      setBusy(false);
    }
  }

  async function decidePermission(request: PermissionRequest, approved: boolean) {
    setBusy(true);
    try {
      await api.decidePermission(request.id, approved);
      setSnapshot((current) =>
        current ? { ...current, approvals: current.approvals.filter((approval) => approval.id !== request.id) } : current,
      );
      setNotice(approved ? '远程请求已通过' : '远程请求已拒绝');
    } finally {
      setBusy(false);
    }
  }

  async function simulateConnectorMessage(message: ConnectorFormValue) {
    setBusy(true);
    try {
      const approval = await api.ingestConnectorMessage(message);
      setSnapshot((current) =>
        current ? { ...current, approvals: [approval, ...current.approvals.filter((item) => item.id !== approval.id)] } : current,
      );
      setActive('approvals');
      setNotice('连接器消息已进入本机审批队列');
    } finally {
      setBusy(false);
    }
  }

  const page = pageCopy[active];
  const content = useMemo(() => {
    if (!snapshot) {
      return <LoadingPanel />;
    }
    const agentName = makeAgentNameLookup(snapshot.agents);
    const agentCapabilities = makeCapabilityLookup(snapshot.capabilities);

    if (active === 'agents') {
      return (
        <AgentsWorkspace
          agents={snapshot.agents}
          capabilities={agentCapabilities}
          selectedAgents={selectedAgents}
          busy={busy}
          onToggleAgent={(id) => setSelectedAgents((current) => toggle(current, id))}
          onAddManual={addManualAgent}
          onRefreshAgents={refreshAgents}
        />
      );
    }
    if (active === 'tasks') {
      return (
        <TasksWorkspace
          agents={snapshot.agents}
          selectedAgents={selectedAgents}
          title={title}
          prompt={prompt}
          busy={busy}
          onToggleAgent={(id) => setSelectedAgents((current) => toggle(current, id))}
          onTitleChange={setTitle}
          onPromptChange={setPrompt}
          onSubmit={submitTask}
        />
      );
    }
    if (active === 'sessions') {
      return <SessionsWorkspace tasks={snapshot.tasks} agentName={agentName} />;
    }
    if (active === 'capabilities') {
      return (
        <CapabilitiesWorkspace
          capabilities={snapshot.capabilities}
          agents={snapshot.agents}
          agentName={agentName}
          onRefresh={refreshCapabilities}
        />
      );
    }
    if (active === 'approvals') {
      return <ApprovalsWorkspace approvals={snapshot.approvals} onDecide={decidePermission} />;
    }
    if (active === 'connectors') {
      return <ConnectorsWorkspace busy={busy} onSimulate={simulateConnectorMessage} />;
    }
    return <SettingsWorkspace desktopRuntime={desktopRuntime} agents={snapshot.agents} />;
  }, [active, busy, desktopRuntime, prompt, selectedAgents, snapshot, title]);

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand">
          <div className="brand-mark">M</div>
          <div>
            <strong>Multica</strong>
            <span>本地 agent 控制台</span>
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
          <span>{desktopRuntime ? '本地进程模式' : 'Web 预览模式'}</span>
        </div>
      </aside>

      <main className="main-surface">
        <header className="topbar">
          <div>
            <h1>{page.title}</h1>
            <p>{page.description}</p>
          </div>
          <div className="topbar-actions">
            <span className={desktopRuntime ? 'runtime-badge desktop' : 'runtime-badge web'}>
              {desktopRuntime ? '桌面模式' : 'Web 预览'}
            </span>
            <button className="icon-button" type="button" title="刷新控制台" onClick={() => loadSnapshot(false)} disabled={busy}>
              <RefreshCw size={18} aria-hidden="true" />
            </button>
            <button className="primary-button" type="button" onClick={() => setActive('tasks')}>
              <Play size={17} aria-hidden="true" />
              新建任务
            </button>
          </div>
        </header>

        {!desktopRuntime ? <RuntimeModeBanner /> : null}
        {notice ? <Notice message={notice} onClose={() => setNotice(null)} /> : null}
        {content}
      </main>
    </div>
  );
}

function AgentsWorkspace({
  agents,
  capabilities,
  selectedAgents,
  busy,
  onToggleAgent,
  onAddManual,
  onRefreshAgents,
}: {
  agents: AgentProfile[];
  capabilities: Map<string, CapabilityInventory>;
  selectedAgents: string[];
  busy: boolean;
  onToggleAgent: (id: string) => void;
  onAddManual: (name: string, command: string, args: string[]) => void;
  onRefreshAgents: () => void;
}) {
  const selected = agents.find((agent) => selectedAgents.includes(agent.id)) ?? agents[0];
  const inventory = selected ? capabilities.get(selected.id) : undefined;

  return (
    <section className="workspace-split">
      <div className="workspace-main">
        <ToolbarPanel
          icon={Bot}
          title="本机 agent"
          action={`${agents.length} 个已登记`}
          controls={
            <button className="secondary-button" type="button" onClick={onRefreshAgents} disabled={busy}>
              <RefreshCw size={16} aria-hidden="true" />
              刷新发现
            </button>
          }
        >
          <ManualAgentForm busy={busy} onSubmit={onAddManual} />
          <div className="agent-table">
            {agents.map((agent) => (
              <AgentRow
                key={agent.id}
                agent={agent}
                selected={selectedAgents.includes(agent.id)}
                onClick={() => onToggleAgent(agent.id)}
              />
            ))}
            {agents.length === 0 ? <EmptyState label="未发现本机 agent，请刷新或手动添加启动命令" /> : null}
          </div>
        </ToolbarPanel>
      </div>
      <aside className="workspace-side">
        <PanelHeader icon={Search} title="探测详情" action={selected ? selected.id : '未选择'} />
        {selected ? (
          <div className="detail-list">
            <DetailLine label="名称" value={selected.name} />
            <DetailLine label="运行方式" value={formatRuntime(selected.runtime)} />
            <DetailLine label="启动命令" value={selected.launch.command} />
            <DetailLine label="默认参数" value={selected.launch.args.join(' ') || '无'} />
            <DetailLine label="检测路径" value={selected.detectedPath ?? '未记录'} />
            <DetailLine label="版本" value={selected.version ?? '未探测'} />
            <DetailLine label="能力" value={`${inventory?.models.length ?? 0} 模型 / ${inventory?.skills.length ?? 0} 技能 / ${inventory?.mcpServers.length ?? 0} MCP`} />
          </div>
        ) : (
          <EmptyState label="选择一个 agent 查看详情" />
        )}
      </aside>
    </section>
  );
}

function TasksWorkspace({
  agents,
  selectedAgents,
  title,
  prompt,
  busy,
  onToggleAgent,
  onTitleChange,
  onPromptChange,
  onSubmit,
}: {
  agents: AgentProfile[];
  selectedAgents: string[];
  title: string;
  prompt: string;
  busy: boolean;
  onToggleAgent: (id: string) => void;
  onTitleChange: (value: string) => void;
  onPromptChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <section className="workspace-split">
      <div className="workspace-main">
        <ToolbarPanel icon={TerminalSquare} title="任务输入" action={`${selectedAgents.length} 个 agent`}>
          <form className="task-form large" onSubmit={onSubmit}>
            <label>
              <span>标题</span>
              <input value={title} onChange={(event) => onTitleChange(event.target.value)} />
            </label>
            <label>
              <span>提示词</span>
              <textarea value={prompt} onChange={(event) => onPromptChange(event.target.value)} rows={8} />
            </label>
            <button className="primary-button full" type="submit" disabled={busy || selectedAgents.length === 0 || prompt.trim().length === 0}>
              <Play size={17} aria-hidden="true" />
              并行运行所选 agent
            </button>
          </form>
        </ToolbarPanel>
      </div>
      <aside className="workspace-side">
        <PanelHeader icon={Bot} title="选择运行目标" action="可多选" />
        <div className="compact-agent-list">
          {agents.map((agent) => (
            <AgentRow key={agent.id} agent={agent} selected={selectedAgents.includes(agent.id)} onClick={() => onToggleAgent(agent.id)} compact />
          ))}
        </div>
      </aside>
    </section>
  );
}

function SessionsWorkspace({ tasks, agentName }: { tasks: TaskSummary[]; agentName: (id: string) => string }) {
  return (
    <section className="wide-panel">
      <PanelHeader icon={Clock3} title="运行时间线" action={`${tasks.length} 条最近任务`} />
      <div className="timeline">
        {tasks.length === 0 ? <EmptyState label="还没有运行记录" /> : null}
        {tasks.map((task) => (
          <article className="task-row" key={task.task.id}>
            <div className="task-main">
              <StatusPill status={task.status} label={formatRunStatus(task.status)} />
              <div>
                <strong>{task.task.title}</strong>
                <p>{task.task.prompt}</p>
              </div>
            </div>
            <div className="run-list">
              {task.runs.map((run) => (
                <div className="run-item" key={run.id}>
                  <span>{agentName(run.agentId)}</span>
                  <StatusPill status={run.status} label={formatRunStatus(run.status)} />
                  <p>{run.result || run.error || '无输出'}</p>
                </div>
              ))}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

function CapabilitiesWorkspace({
  capabilities,
  agents,
  agentName,
  onRefresh,
}: {
  capabilities: CapabilityInventory[];
  agents: AgentProfile[];
  agentName: (id: string) => string;
  onRefresh: () => void;
}) {
  return (
    <section className="wide-panel">
      <PanelHeader icon={Layers3} title="能力矩阵" action={`${capabilities.length}/${agents.length} 已扫描`} />
      <button className="secondary-button inventory-refresh" type="button" onClick={onRefresh}>
        <RefreshCw size={16} aria-hidden="true" />
        刷新清单
      </button>
      <div className="capability-grid">
        {capabilities.length === 0 ? <EmptyState label="尚未扫描能力，点击刷新清单开始探测" /> : null}
        {capabilities.map((inventory) => (
          <article key={inventory.agentId} className="capability-card">
            <header>
              <strong>{agentName(inventory.agentId)}</strong>
              <small>{formatAuth(inventory.auth)}</small>
            </header>
            <CapabilityLine label="模型" value={inventory.models.map((model) => model.label).join(', ') || '未发现'} />
            <CapabilityLine label="技能" value={inventory.skills.map((skill) => skill.name).join(', ') || '未发现'} />
            <CapabilityLine label="MCP" value={inventory.mcpServers.map((server) => server.name).join(', ') || '未发现'} />
            <CapabilityLine label="命令" value={inventory.slashCommands.map((command) => command.name).join(', ') || '未发现'} />
          </article>
        ))}
      </div>
    </section>
  );
}

function ApprovalsWorkspace({
  approvals,
  onDecide,
}: {
  approvals: PermissionRequest[];
  onDecide: (request: PermissionRequest, approved: boolean) => void;
}) {
  return (
    <section className="wide-panel">
      <PanelHeader icon={ShieldAlert} title="待审批请求" action={`${approvals.length} 个待处理`} />
      <div className="approval-list">
        {approvals.length === 0 ? <EmptyState label="暂无远程请求或高风险动作" /> : null}
        {approvals.map((approval) => (
          <article className="approval-card expanded" key={approval.id}>
            <div>
              <strong>{approval.summary}</strong>
              <p>{String(approval.details.preview ?? '远程连接器请求')}</p>
              <small>{approval.kind} · {approval.createdAt}</small>
            </div>
            <div className="approval-actions">
              <button type="button" className="icon-button approve" title="通过" onClick={() => onDecide(approval, true)}>
                <Check size={16} aria-hidden="true" />
              </button>
              <button type="button" className="icon-button deny" title="拒绝" onClick={() => onDecide(approval, false)}>
                <X size={16} aria-hidden="true" />
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

type ConnectorFormValue = {
  connector: string;
  tenantId: string;
  conversationId: string;
  senderId: string;
  text: string;
};

function ConnectorsWorkspace({
  busy,
  onSimulate,
}: {
  busy: boolean;
  onSimulate: (value: ConnectorFormValue) => void;
}) {
  const [form, setForm] = useState<ConnectorFormValue>({
    connector: 'feishu',
    tenantId: 'tenant_local',
    conversationId: 'chat_local',
    senderId: 'user_local',
    text: '请让本地 agent 总结这个仓库的主要风险。',
  });

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSimulate(form);
  }

  return (
    <section className="workspace-split">
      <div className="workspace-main">
        <ToolbarPanel icon={MessageSquareText} title="本地模拟入口" action="进入审批队列">
          <form className="task-form" onSubmit={submit}>
            <label>
              <span>连接器</span>
              <select value={form.connector} onChange={(event) => setForm({ ...form, connector: event.target.value })}>
                <option value="feishu">飞书 Bot</option>
                <option value="wecom">企业微信应用</option>
                <option value="local">本地调试</option>
              </select>
            </label>
            <div className="form-grid">
              <label>
                <span>租户</span>
                <input value={form.tenantId} onChange={(event) => setForm({ ...form, tenantId: event.target.value })} />
              </label>
              <label>
                <span>会话</span>
                <input value={form.conversationId} onChange={(event) => setForm({ ...form, conversationId: event.target.value })} />
              </label>
              <label>
                <span>发送者</span>
                <input value={form.senderId} onChange={(event) => setForm({ ...form, senderId: event.target.value })} />
              </label>
            </div>
            <label>
              <span>消息正文</span>
              <textarea value={form.text} onChange={(event) => setForm({ ...form, text: event.target.value })} rows={5} />
            </label>
            <button className="primary-button full" type="submit" disabled={busy || form.text.trim().length === 0}>
              <ShieldAlert size={17} aria-hidden="true" />
              送入本机审批
            </button>
          </form>
        </ToolbarPanel>
      </div>
      <aside className="workspace-side connector-status">
        <PanelHeader icon={Network} title="Relay 接入点" action="MVP" />
        <DetailLine label="健康检查" value="GET /health" />
        <DetailLine label="飞书回调" value="POST /webhooks/feishu/{tenant_id}" />
        <DetailLine label="企微回调" value="POST /webhooks/wecom/{tenant_id}" />
        <DetailLine label="桌面拉取" value="WS /desktop/ws/{tenant_id}" />
        <p className="muted-copy">Relay 只保存路由、状态、短期队列元数据和正文哈希；桌面端审批通过后再运行 agent。</p>
      </aside>
    </section>
  );
}

function SettingsWorkspace({ desktopRuntime, agents }: { desktopRuntime: boolean; agents: AgentProfile[] }) {
  return (
    <section className="wide-panel settings-board">
      <PanelHeader icon={SlidersHorizontal} title="运行策略" action="本地优先" />
      <div className="settings-row">
        <KeyRound size={18} aria-hidden="true" />
        <div>
          <strong>远程请求默认需要本机审批</strong>
          <span>飞书、企业微信和本地模拟消息都会先进入审批队列，通过后才允许运行 agent。</span>
        </div>
      </div>
      <div className="settings-row">
        <Search size={18} aria-hidden="true" />
        <div>
          <strong>发现策略</strong>
          <span>扫描 ACP Registry、Windows PATH、用户 npm bin、.local/bin、Codex 安装目录，并允许手动补充。</span>
        </div>
      </div>
      <div className="settings-row">
        <TerminalSquare size={18} aria-hidden="true" />
        <div>
          <strong>当前运行时</strong>
          <span>{desktopRuntime ? 'Tauri 桌面模式，可以启动本机进程。' : 'Web 预览模式，只能使用 mock 数据。'} 已登记 {agents.length} 个 agent。</span>
        </div>
      </div>
    </section>
  );
}

function ManualAgentForm({ busy, onSubmit }: { busy: boolean; onSubmit: (name: string, command: string, args: string[]) => void }) {
  const [manualName, setManualName] = useState('');
  const [manualCommand, setManualCommand] = useState('');
  const [manualArgs, setManualArgs] = useState('');

  function submitManualAgent(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit(manualName, manualCommand, parseArgumentString(manualArgs));
    setManualName('');
    setManualCommand('');
    setManualArgs('');
  }

  return (
    <form className="manual-agent-form" onSubmit={submitManualAgent}>
      <label>
        <span>名称</span>
        <input value={manualName} onChange={(event) => setManualName(event.target.value)} placeholder="Claude Code 本地" />
      </label>
      <label>
        <span>命令</span>
        <input value={manualCommand} onChange={(event) => setManualCommand(event.target.value)} placeholder="claude" />
      </label>
      <label>
        <span>参数</span>
        <input value={manualArgs} onChange={(event) => setManualArgs(event.target.value)} placeholder="可选参数；留空会自动按 CLI 类型处理" />
      </label>
      <button className="secondary-button" type="submit" disabled={busy}>
        <Plus size={16} aria-hidden="true" />
        添加
      </button>
    </form>
  );
}

function AgentRow({
  agent,
  selected,
  compact = false,
  onClick,
}: {
  agent: AgentProfile;
  selected: boolean;
  compact?: boolean;
  onClick: () => void;
}) {
  return (
    <button type="button" className={selected ? 'agent-row selected' : 'agent-row'} onClick={onClick}>
      <span className="check-slot">{selected ? <Check size={14} /> : null}</span>
      <span>
        <strong>{agent.name}</strong>
        <small>{compact ? formatRuntime(agent.runtime) : agent.launch.command}</small>
      </span>
      <StatusPill status={agent.acp === 'supported' ? 'succeeded' : 'queued'} label={agent.acp === 'supported' ? 'ACP' : formatRuntime(agent.runtime)} />
      <span className="source-chip">{formatAgentSource(agent.source)}</span>
    </button>
  );
}

function ToolbarPanel({
  icon,
  title,
  action,
  controls,
  children,
}: {
  icon: typeof Bot;
  title: string;
  action: string;
  controls?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="wide-panel">
      <div className="toolbar-panel-header">
        <PanelHeader icon={icon} title={title} action={action} />
        {controls}
      </div>
      {children}
    </section>
  );
}

function RuntimeModeBanner() {
  return (
    <section className="runtime-banner">
      <TerminalSquare size={18} aria-hidden="true" />
      <span>当前是 Web 预览，只使用 mock 数据；真实调用本地 agent 需要从 Tauri 桌面模式启动。</span>
    </section>
  );
}

function Notice({ message, onClose }: { message: string; onClose: () => void }) {
  return (
    <div className="notice" role="status">
      <Check size={16} aria-hidden="true" />
      <span>{message}</span>
      <button type="button" onClick={onClose} title="关闭提示">
        <X size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

function PanelHeader({ icon: Icon, title, action }: { icon: typeof Bot; title: string; action: string }) {
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

function DetailLine({ label, value }: { label: string; value: string }) {
  return (
    <p className="detail-line">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </p>
  );
}

function CapabilityLine({ label, value }: { label: string; value: string }) {
  return (
    <p className="capability-line">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </p>
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
      <span>正在加载本地控制平面</span>
    </section>
  );
}

function reconcileSelectedAgents(current: string[], agents: AgentProfile[]) {
  const valid = current.filter((agentId) => agents.some((agent) => agent.id === agentId));
  if (valid.length > 0) {
    return valid;
  }
  const preferred = agents.find((agent) => agent.source !== 'fixture') ?? agents[0];
  return preferred ? [preferred.id] : [];
}

function toggle(values: string[], value: string) {
  return values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
}

function makeAgentNameLookup(agents: AgentProfile[]) {
  return (agentId: string) => agents.find((agent) => agent.id === agentId)?.name ?? agentId;
}

function makeCapabilityLookup(capabilities: CapabilityInventory[]) {
  return new Map(capabilities.map((inventory) => [inventory.agentId, inventory]));
}

function formatRunStatus(status: RunStatus | 'queued') {
  const labels: Record<RunStatus | 'queued', string> = {
    queued: '排队中',
    running: '运行中',
    succeeded: '成功',
    failed: '失败',
    cancelled: '已取消',
    needsApproval: '待审批',
  };
  return labels[status];
}

function formatAgentSource(source: AgentProfile['source']) {
  const labels: Record<AgentProfile['source'], string> = {
    registry: '注册表',
    path: 'PATH',
    commonLocation: '常见位置',
    manual: '手动',
    fixture: '模拟',
  };
  return labels[source];
}

function formatRuntime(runtime: AgentProfile['runtime']) {
  const labels: Record<AgentProfile['runtime'], string> = {
    acpStdio: 'ACP',
    cli: 'CLI',
    fixture: '模拟',
  };
  return labels[runtime] ?? runtime;
}

function formatAuth(auth: unknown) {
  if (typeof auth === 'string') {
    return auth;
  }
  if (auth && typeof auth === 'object') {
    return Object.keys(auth)[0] ?? 'unknown';
  }
  return 'unknown';
}

function parseArgumentString(value: string): string[] {
  const args: string[] = [];
  let current = '';
  let quote: '"' | "'" | null = null;

  for (const char of value.trim()) {
    if ((char === '"' || char === "'") && quote === null) {
      quote = char;
      continue;
    }
    if (char === quote) {
      quote = null;
      continue;
    }
    if (/\s/.test(char) && quote === null) {
      if (current) {
        args.push(current);
        current = '';
      }
      continue;
    }
    current += char;
  }

  if (current) {
    args.push(current);
  }

  return args;
}
