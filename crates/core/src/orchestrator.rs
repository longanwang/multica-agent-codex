use crate::acp::AcpProcessClient;
use crate::store::Store;
use crate::types::{
    AgentLaunch, AgentProfile, AgentRun, AgentRuntime, RunStatus, TaskSpec, TaskSummary,
};
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
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

    let result = match agent.runtime {
        AgentRuntime::Fixture => run_fake_agent(&agent, &prompt).await,
        AgentRuntime::AcpStdio => run_acp_agent(&agent, &prompt).await,
        AgentRuntime::Cli => run_cli_agent(&agent, &prompt).await,
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

async fn run_acp_agent(agent: &AgentProfile, prompt: &str) -> Result<String> {
    tokio::time::timeout(
        Duration::from_secs(900),
        AcpProcessClient::run_prompt(&agent.launch, prompt, agent.launch.cwd.clone()),
    )
    .await
    .context("ACP agent run timed out after 15 minutes")?
    .map(|result| result.text)
}

async fn run_cli_agent(agent: &AgentProfile, prompt: &str) -> Result<String> {
    let mut launch = cli_launch_for_prompt(agent, prompt);
    let output_capture = attach_cli_output_capture(agent, &mut launch);
    let mut command = crate::process::command_with_args(&launch.command, &launch.args);
    if let Some(cwd) = &launch.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &launch.env {
        command.env(key, value);
    }

    let output = tokio::time::timeout(Duration::from_secs(900), command.output())
        .await
        .context("agent CLI run timed out after 15 minutes")?
        .with_context(|| format!("failed to spawn agent CLI {}", launch.command))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        if let Some(path) = output_capture {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let text = text.trim().to_string();
                let _ = std::fs::remove_file(&path);
                if !text.is_empty() {
                    return Ok(text);
                }
            }
        }
        if stdout.is_empty() && !stderr.is_empty() {
            return Ok(stderr);
        }
        return Ok(stdout);
    }

    anyhow::bail!(
        "{} exited with status {}{}{}",
        agent.name,
        output.status,
        if stderr.is_empty() { "" } else { ": " },
        stderr
    )
}

fn cli_launch_for_prompt(agent: &AgentProfile, prompt: &str) -> AgentLaunch {
    let mut launch = agent.launch.clone();
    let command_name = command_stem(&launch.command);

    match agent.id.as_str() {
        "claude-code" if launch.args.is_empty() => {
            launch.args = vec![
                "-p".to_string(),
                "--output-format".to_string(),
                "text".to_string(),
                "--permission-mode".to_string(),
                "plan".to_string(),
                prompt.to_string(),
            ];
        }
        "codex" if launch.args.is_empty() => {
            launch.args = vec![
                "exec".to_string(),
                "--sandbox".to_string(),
                "read-only".to_string(),
                "--color".to_string(),
                "never".to_string(),
                prompt.to_string(),
            ];
        }
        "gemini" if launch.args.is_empty() => {
            launch.args = vec!["-p".to_string(), prompt.to_string()];
        }
        "opencode" if launch.args.is_empty() => {
            launch.args = vec!["run".to_string(), prompt.to_string()];
        }
        _ if launch.args.is_empty() && command_name.contains("claude") => {
            launch.args = vec![
                "-p".to_string(),
                "--output-format".to_string(),
                "text".to_string(),
                prompt.to_string(),
            ];
        }
        _ if launch.args.is_empty() && command_name.contains("codex") => {
            launch.args = vec![
                "exec".to_string(),
                "--sandbox".to_string(),
                "read-only".to_string(),
                "--color".to_string(),
                "never".to_string(),
                prompt.to_string(),
            ];
        }
        _ if launch.args.is_empty() && command_name.contains("gemini") => {
            launch.args = vec!["-p".to_string(), prompt.to_string()];
        }
        _ => {
            launch.args.push(prompt.to_string());
        }
    }

    launch
}

fn attach_cli_output_capture(agent: &AgentProfile, launch: &mut AgentLaunch) -> Option<PathBuf> {
    if !is_codex_launch(agent, &launch.command) {
        return None;
    }
    if launch.args.iter().any(|arg| arg == "--output-last-message") {
        return None;
    }
    if launch.args.first().map(String::as_str) != Some("exec") || launch.args.len() < 2 {
        return None;
    }

    let path = std::env::temp_dir().join(format!("multica-codex-{}.txt", uuid::Uuid::new_v4()));
    let prompt_index = launch.args.len() - 1;
    launch.args.splice(
        prompt_index..prompt_index,
        [
            "--output-last-message".to_string(),
            path.display().to_string(),
        ],
    );
    Some(path)
}

fn is_codex_launch(agent: &AgentProfile, command: &str) -> bool {
    agent.id == "codex" || command_stem(command).contains("codex")
}

fn command_stem(command: &str) -> String {
    Path::new(command)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .to_ascii_lowercase()
}

async fn run_fake_agent(agent: &AgentProfile, prompt: &str) -> Result<String> {
    let mode = agent
        .launch
        .args
        .get(0)
        .map(String::as_str)
        .unwrap_or("success");
    match mode {
        "fail" => anyhow::bail!("模拟智能体按要求返回失败"),
        "timeout" => {
            tokio::time::sleep(Duration::from_millis(250)).await;
            anyhow::bail!("模拟智能体运行超时")
        }
        _ => {
            tokio::time::sleep(Duration::from_millis(40)).await;
            Ok(format!(
                "{} 已完成模拟运行，提示词：{}",
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

    #[tokio::test]
    async fn runs_cli_agent_with_prompt_argument() {
        let store = Store::in_memory().unwrap();
        let mut echo = manual_profile(
            "Echo CLI",
            "node",
            vec![
                "-e".into(),
                "console.log(`echo:${process.argv.slice(1).join(' ')}`)".into(),
            ],
        );
        echo.id = "echo-cli".to_string();
        store.upsert_agent(&echo).unwrap();

        let orchestrator = Orchestrator::new(store);
        let spec = TaskSpec::new(
            "CLI smoke",
            "hello from multica",
            vec!["echo-cli".into()],
            RequestSource::LocalUser,
        );
        let summary = orchestrator.run_task(spec).await.unwrap();
        assert_eq!(summary.status, RunStatus::Succeeded);
        assert_eq!(
            summary.runs[0].result.as_deref(),
            Some("echo:hello from multica")
        );
    }

    #[test]
    fn codex_cli_launch_uses_current_exec_flags() {
        let mut codex = manual_profile("Codex CLI", "codex", vec![]);
        codex.id = "codex".to_string();

        let launch = cli_launch_for_prompt(&codex, "你好");

        assert_eq!(launch.args[0], "exec");
        assert!(launch.args.contains(&"--sandbox".to_string()));
        assert!(launch.args.contains(&"read-only".to_string()));
        assert!(!launch.args.contains(&"--ask-for-approval".to_string()));
        assert_eq!(launch.args.last().map(String::as_str), Some("你好"));
    }

    #[test]
    fn codex_cli_capture_writes_last_message_before_prompt() {
        let mut codex = manual_profile("Codex CLI", "codex", vec![]);
        codex.id = "codex".to_string();
        let mut launch = cli_launch_for_prompt(&codex, "hello");

        let path = attach_cli_output_capture(&codex, &mut launch).unwrap();

        let output_flag = launch
            .args
            .iter()
            .position(|arg| arg == "--output-last-message")
            .unwrap();
        assert_eq!(launch.args[output_flag + 1], path.display().to_string());
        assert!(output_flag < launch.args.len() - 1);
        assert_eq!(launch.args.last().map(String::as_str), Some("hello"));
    }
}
