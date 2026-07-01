use crate::types::{
    AgentProfile, AgentRun, CapabilityInventory, DashboardSnapshot, PermissionDecision,
    PermissionRequest, PermissionStatus, RunStatus, TaskSpec, TaskSummary,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    file_root: PathBuf,
}

impl Store {
    pub fn open(db_path: impl AsRef<Path>, file_root: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        let file_root = file_root.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create SQLite parent directory {}", parent.display())
            })?;
        }
        std::fs::create_dir_all(&file_root)
            .with_context(|| format!("failed to create file root {}", file_root.display()))?;

        let conn = Connection::open(db_path)
            .with_context(|| format!("failed to open SQLite database {}", db_path.display()))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            file_root,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory SQLite store")?;
        let file_root = std::env::temp_dir().join(format!("multica-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&file_root)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            file_root,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn file_root(&self) -> &Path {
        &self.file_root
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS agents (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              profile_json TEXT NOT NULL,
              capability_json TEXT,
              last_seen TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
              id TEXT PRIMARY KEY,
              spec_json TEXT NOT NULL,
              status TEXT NOT NULL,
              created_at TEXT NOT NULL,
              completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS agent_runs (
              id TEXT PRIMARY KEY,
              task_id TEXT NOT NULL,
              agent_id TEXT NOT NULL,
              status TEXT NOT NULL,
              run_json TEXT NOT NULL,
              started_at TEXT NOT NULL,
              completed_at TEXT,
              FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS permissions (
              id TEXT PRIMARY KEY,
              task_id TEXT,
              connector_message_id TEXT,
              status TEXT NOT NULL,
              request_json TEXT NOT NULL,
              created_at TEXT NOT NULL,
              decided_at TEXT
            );

            CREATE TABLE IF NOT EXISTS connector_messages (
              id TEXT PRIMARY KEY,
              connector TEXT NOT NULL,
              tenant_id TEXT NOT NULL,
              conversation_id TEXT NOT NULL,
              sender_id TEXT NOT NULL,
              metadata_json TEXT NOT NULL,
              received_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS inter_agent_messages (
              id TEXT PRIMARY KEY,
              task_id TEXT NOT NULL,
              from_agent_id TEXT NOT NULL,
              to_agent_id TEXT,
              message_json TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_events (
              id TEXT PRIMARY KEY,
              event_type TEXT NOT NULL,
              subject_id TEXT,
              payload_hash TEXT NOT NULL,
              payload_json TEXT,
              created_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_agent(&self, profile: &AgentProfile) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute(
            r#"
            INSERT INTO agents (id, name, profile_json, last_seen)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
              name = excluded.name,
              profile_json = excluded.profile_json,
              last_seen = excluded.last_seen
            "#,
            params![
                profile.id,
                profile.name,
                to_json(profile)?,
                profile.last_seen.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn upsert_agents(&self, profiles: &[AgentProfile]) -> Result<()> {
        for profile in profiles {
            self.upsert_agent(profile)?;
        }
        Ok(())
    }

    pub fn list_agents(&self) -> Result<Vec<AgentProfile>> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        let mut stmt = conn.prepare("SELECT profile_json FROM agents ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| from_json(&row?)).collect()
    }

    pub fn get_agent(&self, agent_id: &str) -> Result<Option<AgentProfile>> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        let value = conn
            .query_row(
                "SELECT profile_json FROM agents WHERE id = ?1",
                params![agent_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        value.map(|json| from_json(&json)).transpose()
    }

    pub fn upsert_capability(&self, inventory: &CapabilityInventory) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute(
            "UPDATE agents SET capability_json = ?1 WHERE id = ?2",
            params![to_json(inventory)?, inventory.agent_id],
        )?;
        Ok(())
    }

    pub fn list_capabilities(&self) -> Result<Vec<CapabilityInventory>> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT capability_json FROM agents WHERE capability_json IS NOT NULL ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| from_json(&row?)).collect()
    }

    pub fn insert_task(&self, spec: &TaskSpec, status: RunStatus) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute(
            r#"
            INSERT INTO tasks (id, spec_json, status, created_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                spec.id,
                to_json(spec)?,
                status_name(&status),
                spec.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_task_status(
        &self,
        task_id: &str,
        status: RunStatus,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute(
            "UPDATE tasks SET status = ?1, completed_at = ?2 WHERE id = ?3",
            params![
                status_name(&status),
                completed_at.map(|dt| dt.to_rfc3339()),
                task_id
            ],
        )?;
        Ok(())
    }

    pub fn upsert_agent_run(&self, run: &AgentRun) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute(
            r#"
            INSERT INTO agent_runs (id, task_id, agent_id, status, run_json, started_at, completed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
              status = excluded.status,
              run_json = excluded.run_json,
              completed_at = excluded.completed_at
            "#,
            params![
                run.id,
                run.task_id,
                run.agent_id,
                status_name(&run.status),
                to_json(run)?,
                run.started_at.to_rfc3339(),
                run.completed_at.map(|dt| dt.to_rfc3339())
            ],
        )?;
        Ok(())
    }

    pub fn list_tasks(&self, limit: usize) -> Result<Vec<TaskSummary>> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        let mut stmt = conn.prepare(
            r#"
            SELECT id, spec_json, status, completed_at
            FROM tasks
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?;
        let task_rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut summaries = Vec::new();
        for row in task_rows {
            let (task_id, spec_json, status, completed_at) = row?;
            let task: TaskSpec = from_json(&spec_json)?;
            let mut runs_stmt = conn.prepare(
                "SELECT run_json FROM agent_runs WHERE task_id = ?1 ORDER BY started_at ASC",
            )?;
            let run_rows = runs_stmt.query_map(params![task_id], |run_row| {
                run_row.get::<_, String>(0)
            })?;
            let runs = run_rows
                .map(|run| from_json::<AgentRun>(&run?))
                .collect::<Result<Vec<_>>>()?;

            summaries.push(TaskSummary {
                task,
                status: parse_status(&status),
                runs,
                completed_at: completed_at
                    .as_deref()
                    .map(DateTime::parse_from_rfc3339)
                    .transpose()?
                    .map(|dt| dt.with_timezone(&Utc)),
            });
        }

        Ok(summaries)
    }

    pub fn insert_permission_request(&self, request: &PermissionRequest) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        conn.execute(
            r#"
            INSERT INTO permissions
              (id, task_id, connector_message_id, status, request_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                request.id,
                request.task_id,
                request.connector_message_id,
                permission_status_name(&request.status),
                to_json(request)?,
                request.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_pending_permissions(&self) -> Result<Vec<PermissionRequest>> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT request_json FROM permissions WHERE status = 'pending' ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| from_json(&row?)).collect()
    }

    pub fn record_permission_decision(&self, decision: &PermissionDecision) -> Result<()> {
        let status = if decision.approved {
            PermissionStatus::Approved
        } else {
            PermissionStatus::Denied
        };
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        let request_json = conn
            .query_row(
                "SELECT request_json FROM permissions WHERE id = ?1",
                params![decision.request_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(request_json) = request_json {
            let mut request: PermissionRequest = from_json(&request_json)?;
            request.status = status.clone();
            conn.execute(
                r#"
                UPDATE permissions
                SET status = ?1, request_json = ?2, decided_at = ?3
                WHERE id = ?4
                "#,
                params![
                    permission_status_name(&status),
                    to_json(&request)?,
                    decision.decided_at.to_rfc3339(),
                    decision.request_id
                ],
            )?;
            self.record_audit_event_locked(
                &conn,
                "permission.decision",
                Some(&decision.request_id),
                decision,
            )?;
        }
        Ok(())
    }

    pub fn record_audit_event<T: Serialize>(
        &self,
        event_type: &str,
        subject_id: Option<&str>,
        payload: &T,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("store connection mutex poisoned");
        self.record_audit_event_locked(&conn, event_type, subject_id, payload)
    }

    fn record_audit_event_locked<T: Serialize>(
        &self,
        conn: &Connection,
        event_type: &str,
        subject_id: Option<&str>,
        payload: &T,
    ) -> Result<()> {
        let payload_json = to_json(payload)?;
        let mut hasher = Sha256::new();
        hasher.update(payload_json.as_bytes());
        let payload_hash = format!("{:x}", hasher.finalize());
        conn.execute(
            r#"
            INSERT INTO audit_events
              (id, event_type, subject_id, payload_hash, payload_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                uuid::Uuid::new_v4().to_string(),
                event_type,
                subject_id,
                payload_hash,
                payload_json,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn dashboard_snapshot(&self) -> Result<DashboardSnapshot> {
        Ok(DashboardSnapshot {
            agents: self.list_agents()?,
            capabilities: self.list_capabilities()?,
            tasks: self.list_tasks(25)?,
            approvals: self.list_pending_permissions()?,
        })
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).context("failed to serialize store payload")
}

fn from_json<T: DeserializeOwned>(json: &str) -> Result<T> {
    serde_json::from_str(json).context("failed to deserialize store payload")
}

fn status_name(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::Succeeded => "succeeded",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
        RunStatus::NeedsApproval => "needs_approval",
    }
}

fn parse_status(value: &str) -> RunStatus {
    match value {
        "queued" => RunStatus::Queued,
        "running" => RunStatus::Running,
        "succeeded" => RunStatus::Succeeded,
        "failed" => RunStatus::Failed,
        "cancelled" => RunStatus::Cancelled,
        "needs_approval" => RunStatus::NeedsApproval,
        _ => RunStatus::Failed,
    }
}

fn permission_status_name(status: &PermissionStatus) -> &'static str {
    match status {
        PermissionStatus::Pending => "pending",
        PermissionStatus::Approved => "approved",
        PermissionStatus::Denied => "denied",
        PermissionStatus::Expired => "expired",
    }
}

