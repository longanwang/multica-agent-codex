use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaunch {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentRuntime {
    AcpStdio,
    Cli,
    Fixture,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::Cli
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AcpSupport {
    Unknown,
    Supported,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentSource {
    Registry,
    Path,
    CommonLocation,
    Manual,
    Fixture,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub launch: AgentLaunch,
    #[serde(default)]
    pub runtime: AgentRuntime,
    pub acp: AcpSupport,
    pub source: AgentSource,
    pub detected_path: Option<String>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityInventory {
    pub agent_id: String,
    #[serde(default)]
    pub models: Vec<ModelOption>,
    #[serde(default)]
    pub skills: Vec<SkillRef>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerRef>,
    #[serde(default)]
    pub slash_commands: Vec<SlashCommandRef>,
    #[serde(default)]
    pub config_options: Vec<ConfigOption>,
    pub auth: AuthState,
    pub scanned_at: DateTime<Utc>,
}

impl CapabilityInventory {
    pub fn empty(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            models: Vec::new(),
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            slash_commands: Vec::new(),
            config_options: Vec::new(),
            auth: AuthState::Unknown,
            scanned_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub source_agent: String,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRef {
    pub id: String,
    pub name: String,
    pub source_agent: String,
    pub source_path: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRef {
    pub id: String,
    pub name: String,
    pub source_agent: String,
    pub source_path: Option<String>,
    pub command: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommandRef {
    pub name: String,
    pub source_agent: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOption {
    pub id: String,
    pub label: String,
    pub category: Option<String>,
    pub value_type: String,
    #[serde(default)]
    pub choices: Vec<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuthState {
    Unknown,
    Ready,
    NeedsLogin,
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RequestSource {
    LocalUser,
    Connector { connector: String, external_user: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpec {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub agent_ids: Vec<String>,
    pub requested_by: RequestSource,
    pub created_at: DateTime<Utc>,
}

impl TaskSpec {
    pub fn new(
        title: impl Into<String>,
        prompt: impl Into<String>,
        agent_ids: Vec<String>,
        requested_by: RequestSource,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            prompt: prompt.into(),
            agent_ids,
            requested_by,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    NeedsApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    pub task_id: String,
    pub agent_id: String,
    pub status: RunStatus,
    pub transcript_path: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl AgentRun {
    pub fn started(task_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.into(),
            agent_id: agent_id.into(),
            status: RunStatus::Running,
            transcript_path: None,
            result: None,
            error: None,
            started_at: Utc::now(),
            completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub task: TaskSpec,
    pub status: RunStatus,
    #[serde(default)]
    pub runs: Vec<AgentRun>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InterAgentMessage {
    pub id: String,
    pub task_id: String,
    pub from_agent_id: String,
    pub to_agent_id: Option<String>,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    FileRead,
    FileWrite,
    Terminal,
    Network,
    ConnectorReply,
    AgentMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub id: String,
    pub task_id: Option<String>,
    pub connector_message_id: Option<String>,
    pub kind: PermissionKind,
    pub summary: String,
    pub details: Value,
    pub status: PermissionStatus,
    pub created_at: DateTime<Utc>,
}

impl PermissionRequest {
    pub fn pending(kind: PermissionKind, summary: impl Into<String>, details: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: None,
            connector_message_id: None,
            kind,
            summary: summary.into(),
            details,
            status: PermissionStatus::Pending,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDecision {
    pub request_id: String,
    pub approved: bool,
    pub decided_by: String,
    pub reason: Option<String>,
    pub decided_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorMessage {
    pub id: String,
    pub connector: String,
    pub tenant_id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub text: String,
    pub raw: Value,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutboundReply {
    pub connector: String,
    pub tenant_id: String,
    pub conversation_id: String,
    pub text: String,
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub agents: Vec<AgentProfile>,
    pub capabilities: Vec<CapabilityInventory>,
    pub tasks: Vec<TaskSummary>,
    pub approvals: Vec<PermissionRequest>,
}
