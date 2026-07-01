use crate::acp::AcpProcessClient;
use crate::store::Store;
use crate::types::{AgentProfile, AgentRun, RunStatus, TaskSpec, TaskSummary};
use anyhow::{Context, Result};
use chrono::Utc;
use std::time::Duration;
use tokio::task::JoinSet;

#[derive(Clone)]
pub struct Orchestrator {
    store: Store,
}

impl Orchestrator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub async fn run_task(&self, spec: TaskSpec) -> Result<TaskSummary> {
        self.store.insert_task(&spec, RunStatus::Running)?;
        let agents = self.resolve_agents(&spec.agent_ids)?;
        if agents.is_empty() {
            anyhow::bail!("no runnable agents selected");
        }

        let mut runs = JoinSet::new();
        for agent in agents {
            let prompt = spec.prompt.clone();
            let task_id = spec.id.clone();
            let store = self.store.clone();
            runs.spawn(async move { run_single_agent(store, task_id, agent, prompt).await });
        }

        let mut completed = Vec::new();
        while let Some(result) = runs.join_next().await {
            completed.push(result.context("agent run task panicked")??);
        }

        let status = if completed
            .iter()
            .any(|run| matches!(&run.status, RunStatus::Succeeded))
        {
            RunStatus::Succeeded
        } else {
            RunStatus::Failed
        };
        let completed_at = Utc::now();
        self.store
            .update_task_status(&spec.id, status.clone(), Some(completed_at))?;
        Ok(TaskSummary {
            task: spec,
            status,
            runs: completed,
            completed_at: Some(completed_at),
        })
    }

    fn resolve_agents(&self, agent_ids: &[String]) -> Result<Vec<AgentProfile>> {
        let mut agents = Vec::new();
        for agent_id in agent_ids {
            if let Some(agent) = self.store.get_agent(agent_id)? {
                agents.push(agent);
            }
        }
        Ok(agents)
    }
}

async fn run_single_agent(
    store: Store,
    task_id: String,
    agent: AgentProfile,
    prompt: String,
) -> Result<AgentRun> {
    let mut run = AgentRun::started(task_id, agent.id.clone());
    store.upsert_agent_run(&run)?;

    let result = if agent.launch.command == "multica-fake" {
        run_fake_agent(&agent, &prompt).await
    } else {
        AcpProcessClient::run_prompt(&agent.launch, &prompt, agent.launch.cwd.clone())
            .await
            .map(|result| result.text)
    };

    run.completed_at = Some(Utc::now());
    match result {
        Ok(text) => {
            run.status = RunStatus::Succeeded;
            run.result = Some(text);
        }
        Err(error) => {
            run.status = RunStatus::Failed;
            run.error = Some(error.to_string());
        }
    }
    store.upsert_agent_run(&run)?;
    Ok(run)
}

async fn run_fake_agent(agent: &AgentProfile, prompt: &str) -> Result<String> {
    let mode = agent.launch.args.get(0).map(String::as_str).unwrap_or("success");
    match mode {
        "fail" => anyhow::bail!("fixture agent was asked to fail"),
        "timeout" => {
            tokio::time::sleep(Duration::from_millis(250)).await;
            anyhow::bail!("fixture agent timed out")
        }
        _ => {
            tokio::time::sleep(Duration::from_millis(40)).await;
            Ok(format!(
                "{} completed fixture run for prompt: {}",
                agent.name,
                prompt.chars().take(120).collect::<String>()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{fake_agent_profile, manual_profile};
    use crate::types::RequestSource;

    #[tokio::test]
    async fn aggregates_success_and_failure() {
        let store = Store::in_memory().unwrap();
        let mut success = fake_agent_profile();
        success.id = "success".to_string();
        let mut failure = manual_profile("Failing Fixture", "multica-fake", vec!["fail".into()]);
        failure.id = "failure".to_string();
        store.upsert_agent(&success).unwrap();
        store.upsert_agent(&failure).unwrap();

        let orchestrator = Orchestrator::new(store);
        let spec = TaskSpec::new(
            "Review",
            "Check the repository",
            vec!["success".into(), "failure".into()],
            RequestSource::LocalUser,
        );
        let summary = orchestrator.run_task(spec).await.unwrap();
        assert_eq!(summary.status, RunStatus::Succeeded);
        assert_eq!(summary.runs.len(), 2);
        assert!(summary
            .runs
            .iter()
            .any(|run| matches!(run.status, RunStatus::Failed)));
    }
}
