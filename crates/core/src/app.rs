use crate::capabilities::CapabilityScanner;
use crate::discovery::{manual_profile, AgentDiscovery};
use crate::orchestrator::Orchestrator;
use crate::permissions::PermissionBroker;
use crate::store::Store;
use crate::types::{
    AgentProfile, CapabilityInventory, ConnectorMessage, DashboardSnapshot, PermissionRequest,
    RequestSource, TaskSpec, TaskSummary,
};
use anyhow::Result;
use chrono::Utc;
use serde_json::Value;
use std::path::Path;

#[derive(Clone)]
pub struct MulticaCore {
    store: Store,
    discovery: AgentDiscovery,
    scanner: CapabilityScanner,
    orchestrator: Orchestrator,
    permissions: PermissionBroker,
}

impl MulticaCore {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref();
        let store = Store::open(data_dir.join("multica.db"), data_dir.join("files"))?;
        Ok(Self::from_store(store))
    }

    pub fn from_store(store: Store) -> Self {
        let orchestrator = Orchestrator::new(store.clone());
        let permissions = PermissionBroker::new(store.clone());
        Self {
            store,
            discovery: AgentDiscovery::default(),
            scanner: CapabilityScanner::default(),
            orchestrator,
            permissions,
        }
    }

    pub fn store(&self) -> Store {
        self.store.clone()
    }

    pub async fn refresh_agents(&self) -> Result<Vec<AgentProfile>> {
        let agents = self.discovery.discover().await?;
        self.store.upsert_agents(&agents)?;
        Ok(agents)
    }

    pub fn add_manual_agent(
        &self,
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
    ) -> Result<AgentProfile> {
        let profile = manual_profile(name, command, args);
        self.store.upsert_agent(&profile)?;
        Ok(profile)
    }

    pub async fn refresh_capabilities(&self) -> Result<Vec<CapabilityInventory>> {
        let agents = self.store.list_agents()?;
        let mut inventories = Vec::new();
        for agent in agents {
            let inventory = self.scanner.scan_agent(&agent).await;
            self.store.upsert_capability(&inventory)?;
            inventories.push(inventory);
        }
        Ok(inventories)
    }

    pub async fn create_task(
        &self,
        title: impl Into<String>,
        prompt: impl Into<String>,
        agent_ids: Vec<String>,
    ) -> Result<TaskSummary> {
        let spec = TaskSpec::new(title, prompt, agent_ids, RequestSource::LocalUser);
        self.orchestrator.run_task(spec).await
    }

    pub fn ingest_connector_message(
        &self,
        connector: impl Into<String>,
        tenant_id: impl Into<String>,
        conversation_id: impl Into<String>,
        sender_id: impl Into<String>,
        text: impl Into<String>,
        raw: Value,
    ) -> Result<PermissionRequest> {
        let message = ConnectorMessage {
            id: uuid::Uuid::new_v4().to_string(),
            connector: connector.into(),
            tenant_id: tenant_id.into(),
            conversation_id: conversation_id.into(),
            sender_id: sender_id.into(),
            text: text.into(),
            raw,
            received_at: Utc::now(),
        };
        self.permissions.request_connector_task(&message)
    }

    pub fn decide_permission(
        &self,
        request_id: impl Into<String>,
        approved: bool,
        decided_by: impl Into<String>,
    ) -> Result<()> {
        self.permissions
            .decide(request_id.into(), approved, decided_by.into())
    }

    pub fn dashboard_snapshot(&self) -> Result<DashboardSnapshot> {
        self.store.dashboard_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    #[tokio::test]
    async fn core_refreshes_and_runs_fixture_agent() {
        let core = MulticaCore::from_store(Store::in_memory().unwrap());
        let agents = core.refresh_agents().await.unwrap();
        assert!(agents.iter().any(|agent| agent.id == "fixture-fast-reviewer"));
        let summary = core
            .create_task(
                "Smoke",
                "Prove the orchestrator works",
                vec!["fixture-fast-reviewer".into()],
            )
            .await
            .unwrap();
        assert!(summary.runs[0]
            .result
            .as_ref()
            .unwrap()
            .contains("Fixture Fast Reviewer"));
    }
}

