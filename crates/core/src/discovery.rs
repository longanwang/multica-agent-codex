use crate::types::{AcpSupport, AgentLaunch, AgentProfile, AgentRuntime, AgentSource};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const REGISTRY_URL: &str = "https://agentclientprotocol.com/registry/agents.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryAgent {
    pub id: Option<String>,
    pub name: String,
    pub command: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentDiscovery {
    registry_url: String,
}

impl Default for AgentDiscovery {
    fn default() -> Self {
        Self {
            registry_url: REGISTRY_URL.to_string(),
        }
    }
}

impl AgentDiscovery {
    pub fn new(registry_url: impl Into<String>) -> Self {
        Self {
            registry_url: registry_url.into(),
        }
    }

    pub async fn discover(&self) -> Result<Vec<AgentProfile>> {
        let mut profiles = Vec::new();
        let mut seen = BTreeSet::new();

        for registry_agent in self.load_registry_candidates().await.unwrap_or_default() {
            if let Some(command) = registry_agent.command {
                let id = registry_agent
                    .id
                    .clone()
                    .unwrap_or_else(|| slug(&registry_agent.name));
                if let Some(path) = resolve_command(&command) {
                    let profile = profile_from_command(
                        &id,
                        &registry_agent.name,
                        &command,
                        Vec::new(),
                        AgentRuntime::AcpStdio,
                        AcpSupport::Unknown,
                        Some(path),
                        AgentSource::Registry,
                    );
                    seen.insert(profile.id.clone());
                    profiles.push(profile);
                }
            }
        }

        for candidate in known_agent_candidates() {
            if let Some(path) = resolve_command(candidate.command) {
                let profile = profile_from_command(
                    candidate.id,
                    candidate.display_name,
                    &path.display().to_string(),
                    candidate.args.iter().map(|arg| (*arg).to_string()).collect(),
                    candidate.runtime.clone(),
                    candidate.acp.clone(),
                    Some(path),
                    AgentSource::Path,
                );
                if seen.insert(profile.id.clone()) {
                    profiles.push(profile);
                }
            }
        }

        profiles.push(fake_agent_profile());
        Ok(profiles)
    }

    pub async fn load_registry_candidates(&self) -> Result<Vec<RegistryAgent>> {
        let response = reqwest::get(&self.registry_url)
            .await
            .with_context(|| format!("failed to fetch ACP registry {}", self.registry_url))?;
        if !response.status().is_success() {
            anyhow::bail!(
                "ACP registry returned HTTP {} for {}",
                response.status(),
                self.registry_url
            );
        }
        let value = response
            .json::<serde_json::Value>()
            .await
            .context("failed to parse ACP registry JSON")?;
        parse_registry_agents(value)
    }
}

fn parse_registry_agents(value: serde_json::Value) -> Result<Vec<RegistryAgent>> {
    if value.is_array() {
        return Ok(serde_json::from_value(value)?);
    }
    if let Some(agents) = value.get("agents") {
        return Ok(serde_json::from_value(agents.clone())?);
    }
    Ok(Vec::new())
}

struct KnownAgent {
    id: &'static str,
    display_name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    runtime: AgentRuntime,
    acp: AcpSupport,
}

fn known_agent_candidates() -> Vec<KnownAgent> {
    vec![
        KnownAgent {
            id: "opencode",
            display_name: "OpenCode",
            command: "opencode",
            args: &["acp"],
            runtime: AgentRuntime::AcpStdio,
            acp: AcpSupport::Supported,
        },
        KnownAgent {
            id: "claude-code",
            display_name: "Claude Code",
            command: "claude",
            args: &[],
            runtime: AgentRuntime::Cli,
            acp: AcpSupport::Unavailable {
                reason: "已发现 Claude Code CLI；当前使用官方 -p 非交互式模式运行。".to_string(),
            },
        },
        KnownAgent {
            id: "gemini",
            display_name: "Gemini CLI",
            command: "gemini",
            args: &[],
            runtime: AgentRuntime::Cli,
            acp: AcpSupport::Unavailable {
                reason: "已发现 Gemini CLI；当前使用官方 -p/--prompt headless 模式运行。".to_string(),
            },
        },
        KnownAgent {
            id: "codex",
            display_name: "Codex CLI",
            command: "codex",
            args: &[],
            runtime: AgentRuntime::Cli,
            acp: AcpSupport::Unavailable {
                reason: "已发现 Codex CLI；当前使用 codex exec 非交互式模式运行。".to_string(),
            },
        },
    ]
}

fn resolve_command(command: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(command);
    if direct.exists() {
        return Some(direct);
    }
    if let Ok(path) = which::which(command) {
        return Some(path);
    }

    for base in common_windows_roots() {
        for candidate in command_candidates(&base, command) {
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn common_windows_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        roots.push(home.join(".local").join("bin"));
        roots.push(home.join("AppData").join("Roaming").join("npm"));
        roots.push(
            home.join("AppData")
                .join("Local")
                .join("OpenAI")
                .join("Codex")
                .join("bin"),
        );
    }
    for key in ["LOCALAPPDATA", "APPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(value) = std::env::var_os(key) {
            roots.push(PathBuf::from(value));
        }
    }
    roots
}

fn command_candidates(base: &Path, command: &str) -> Vec<PathBuf> {
    let names = if Path::new(command).extension().is_some() {
        vec![command.to_string()]
    } else {
        vec![
            command.to_string(),
            format!("{command}.exe"),
            format!("{command}.cmd"),
            format!("{command}.bat"),
            format!("{command}.ps1"),
        ]
    };

    let mut candidates = Vec::new();
    for name in names {
        candidates.push(base.join(&name));
        candidates.push(base.join("bin").join(&name));
    }
    candidates
}

fn profile_from_command(
    id: &str,
    display_name: &str,
    command: &str,
    args: Vec<String>,
    runtime: AgentRuntime,
    acp: AcpSupport,
    detected_path: Option<PathBuf>,
    source: AgentSource,
) -> AgentProfile {
    let version = detect_version(command);
    AgentProfile {
        id: id.to_string(),
        name: display_name.to_string(),
        version,
        launch: AgentLaunch {
            command: command.to_string(),
            args,
            env: BTreeMap::new(),
            cwd: None,
        },
        runtime,
        acp,
        source,
        detected_path: detected_path.map(|path| path.display().to_string()),
        last_seen: Utc::now(),
    }
}

pub fn manual_profile(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> AgentProfile {
    let name = name.into();
    let command = command.into();
    AgentProfile {
        id: format!("manual-{}", slug(&name)),
        name,
        version: None,
        launch: AgentLaunch {
            command,
            args,
            env: BTreeMap::new(),
            cwd: None,
        },
        runtime: AgentRuntime::Cli,
        acp: AcpSupport::Unknown,
        source: AgentSource::Manual,
        detected_path: None,
        last_seen: Utc::now(),
    }
}

pub fn fake_agent_profile() -> AgentProfile {
    AgentProfile {
        id: "fixture-fast-reviewer".to_string(),
        name: "模拟快速审阅器".to_string(),
        version: Some("0.1.0".to_string()),
        launch: AgentLaunch {
            command: "multica-fake".to_string(),
            args: vec!["success".to_string()],
            env: BTreeMap::new(),
            cwd: None,
        },
        runtime: AgentRuntime::Fixture,
        acp: AcpSupport::Supported,
        source: AgentSource::Fixture,
        detected_path: None,
        last_seen: Utc::now(),
    }
}

fn detect_version(command: &str) -> Option<String> {
    let args = vec!["--version".to_string()];
    let output = std::process::Command::new(command)
        .args(&args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn slug(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_registry_object_or_array() {
        let agents = parse_registry_agents(json!({
            "agents": [
                {"id": "demo", "name": "Demo", "command": "demo"}
            ]
        }))
        .unwrap();
        assert_eq!(agents[0].name, "Demo");

        let agents = parse_registry_agents(json!([
            {"id": "demo2", "name": "Demo 2", "command": "demo2"}
        ]))
        .unwrap();
        assert_eq!(agents[0].id.as_deref(), Some("demo2"));
    }

    #[test]
    fn creates_stable_manual_profile_id() {
        let profile = manual_profile("Claude Code Local", "claude", vec![]);
        assert_eq!(profile.id, "manual-claude-code-local");
    }
}
