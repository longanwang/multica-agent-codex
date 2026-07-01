use crate::types::{
    AgentProfile, AuthState, CapabilityInventory, ConfigOption, McpServerRef, ModelOption,
    SkillRef, SlashCommandRef,
};
use anyhow::{Context, Result};
use chrono::Utc;
use glob::glob;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CapabilityScanner {
    home_dir: PathBuf,
}

impl Default for CapabilityScanner {
    fn default() -> Self {
        Self {
            home_dir: std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
        }
    }
}

impl CapabilityScanner {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        Self {
            home_dir: home_dir.into(),
        }
    }

    pub async fn scan_agent(&self, agent: &AgentProfile) -> CapabilityInventory {
        let mut inventory = CapabilityInventory::empty(agent.id.clone());
        inventory.auth = AuthState::Unknown;
        inventory.scanned_at = Utc::now();

        inventory.models.extend(self.models_from_known_agent(agent));
        inventory.skills.extend(self.scan_skills(agent).unwrap_or_default());
        inventory
            .mcp_servers
            .extend(self.scan_mcp_servers(agent).unwrap_or_default());
        inventory
            .slash_commands
            .extend(self.scan_slash_commands(agent).unwrap_or_default());

        if let Ok(config_options) = self.config_options_from_probe(agent).await {
            for option in config_options {
                if option.category.as_deref() == Some("model") {
                    inventory.models.push(ModelOption {
                        id: option.id.clone(),
                        label: option.label.clone(),
                        source_agent: agent.id.clone(),
                        provider: None,
                    });
                }
                inventory.config_options.push(option);
            }
        }

        dedupe_inventory(&mut inventory);
        inventory
    }

    fn models_from_known_agent(&self, agent: &AgentProfile) -> Vec<ModelOption> {
        let candidates: &[(&str, &str)] = match agent.id.as_str() {
            "fixture-fast-reviewer" => &[
                ("fixture-small", "Fixture Small"),
                ("fixture-large", "Fixture Large"),
            ],
            "claude-code" => &[
                ("claude-opus-4", "Claude Opus 4"),
                ("claude-sonnet-4", "Claude Sonnet 4"),
            ],
            "gemini" => &[("gemini-2.5-pro", "Gemini 2.5 Pro")],
            "codex" => &[("gpt-5-codex", "GPT-5 Codex")],
            _ => &[],
        };
        candidates
            .iter()
            .map(|(id, label)| ModelOption {
                id: (*id).to_string(),
                label: (*label).to_string(),
                source_agent: agent.id.clone(),
                provider: provider_from_agent_id(&agent.id),
            })
            .collect()
    }

    fn scan_skills(&self, agent: &AgentProfile) -> Result<Vec<SkillRef>> {
        let patterns = match agent.id.as_str() {
            "codex" => vec![
                self.home_dir.join(".codex").join("skills").join("*").join("SKILL.md"),
                self.home_dir
                    .join(".codex")
                    .join("plugins")
                    .join("cache")
                    .join("*")
                    .join("skills")
                    .join("*")
                    .join("SKILL.md"),
            ],
            "claude-code" => vec![self
                .home_dir
                .join(".claude")
                .join("skills")
                .join("*")
                .join("SKILL.md"),
                self.home_dir
                    .join(".claude")
                    .join("commands")
                    .join("*.md")],
            "gemini" => vec![
                self.home_dir
                    .join(".gemini")
                    .join("extensions")
                    .join("*")
                    .join("GEMINI.md"),
                self.home_dir
                    .join(".gemini")
                    .join("commands")
                    .join("*.toml"),
            ],
            "opencode" => vec![self
                .home_dir
                .join(".config")
                .join("opencode")
                .join("skills")
                .join("*")
                .join("*.md")],
            "fixture-fast-reviewer" => vec![PathBuf::from("crates/core/tests/fixtures/skills/*/SKILL.md")],
            _ => Vec::new(),
        };

        let mut skills = Vec::new();
        for pattern in patterns {
            for entry in glob(&pattern.to_string_lossy())? {
                let path = entry?;
                skills.push(skill_from_path(agent, &path)?);
            }
        }
        Ok(skills)
    }

    fn scan_mcp_servers(&self, agent: &AgentProfile) -> Result<Vec<McpServerRef>> {
        let patterns = match agent.id.as_str() {
            "codex" => vec![self.home_dir.join(".codex").join("config.toml")],
            "claude-code" => vec![
                self.home_dir.join(".claude").join("mcp.json"),
                self.home_dir.join(".claude").join("settings.json"),
                self.home_dir.join(".claude.json"),
            ],
            "gemini" => vec![self.home_dir.join(".gemini").join("settings.json")],
            "opencode" => vec![
                self.home_dir
                    .join(".config")
                    .join("opencode")
                    .join("opencode.json"),
                self.home_dir
                    .join("AppData")
                    .join("Roaming")
                    .join("opencode")
                    .join("opencode.json"),
            ],
            "fixture-fast-reviewer" => vec![PathBuf::from("crates/core/tests/fixtures/mcp/*.json")],
            _ => Vec::new(),
        };

        let mut servers = Vec::new();
        for path in patterns {
            if path.exists() {
                servers.extend(mcp_servers_from_file(agent, &path).unwrap_or_default());
            }
        }
        Ok(servers)
    }

    fn scan_slash_commands(&self, agent: &AgentProfile) -> Result<Vec<SlashCommandRef>> {
        let commands = match agent.id.as_str() {
            "fixture-fast-reviewer" => vec![SlashCommandRef {
                name: "/review".to_string(),
                source_agent: agent.id.clone(),
                description: Some("运行一次确定性的模拟审阅".to_string()),
            }],
            "codex" => vec![SlashCommandRef {
                name: "/compact".to_string(),
                source_agent: agent.id.clone(),
                description: Some("Compact the active conversation context".to_string()),
            }],
            "claude-code" => vec![
                SlashCommandRef {
                    name: "/config".to_string(),
                    source_agent: agent.id.clone(),
                    description: Some("打开或修改 Claude Code 设置".to_string()),
                },
                SlashCommandRef {
                    name: "/mcp".to_string(),
                    source_agent: agent.id.clone(),
                    description: Some("管理 Claude Code MCP 连接".to_string()),
                },
            ],
            "gemini" => vec![SlashCommandRef {
                name: "/help".to_string(),
                source_agent: agent.id.clone(),
                description: Some("查看 Gemini CLI 会话命令".to_string()),
            }],
            _ => Vec::new(),
        };
        Ok(commands)
    }

    async fn config_options_from_probe(&self, agent: &AgentProfile) -> Result<Vec<ConfigOption>> {
        if agent.launch.command == "multica-fake" {
            return Ok(vec![
                ConfigOption {
                    id: "fixture-small".to_string(),
                    label: "Fixture Small".to_string(),
                    category: Some("model".to_string()),
                    value_type: "select".to_string(),
                    choices: vec!["fixture-small".to_string(), "fixture-large".to_string()],
                    raw: serde_json::json!({"category": "model"}),
                },
                ConfigOption {
                    id: "approval-mode".to_string(),
                    label: "审批模式".to_string(),
                    category: Some("permission".to_string()),
                    value_type: "select".to_string(),
                    choices: vec!["ask".to_string(), "deny".to_string()],
                    raw: serde_json::json!({"category": "permission"}),
                },
            ]);
        }

        let mut args = agent.launch.args.clone();
        args.push("--help".to_string());
        let output = crate::process::command_with_args(&agent.launch.command, &args)
            .output()
            .await
            .with_context(|| format!("failed to probe agent {}", agent.name))?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        Ok(Vec::new())
    }
}

fn skill_from_path(agent: &AgentProfile, path: &Path) -> Result<SkillRef> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read skill {}", path.display()))?;
    let name = extract_frontmatter_field(&content, "name")
        .or_else(|| {
            path.parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "unknown-skill".to_string());
    let description = extract_frontmatter_field(&content, "description");
    Ok(SkillRef {
        id: format!("{}:{}", agent.id, crate::discovery::slug(&name)),
        name,
        source_agent: agent.id.clone(),
        source_path: Some(path.display().to_string()),
        description,
    })
}

fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()? != "---" {
        return None;
    }
    let prefix = format!("{key}:");
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn mcp_servers_from_file(agent: &AgentProfile, path: &Path) -> Result<Vec<McpServerRef>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read MCP config {}", path.display()))?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
        return mcp_servers_from_toml(agent, path, &content);
    }
    let json: Value = serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));
    let mut servers = Vec::new();

    if let Some(map) = json.get("mcpServers").and_then(|value| value.as_object()) {
        for (name, value) in map {
            servers.push(McpServerRef {
                id: format!("{}:{}", agent.id, crate::discovery::slug(name)),
                name: name.clone(),
                source_agent: agent.id.clone(),
                source_path: Some(path.display().to_string()),
                command: value
                    .get("command")
                    .and_then(|command| command.as_str())
                    .map(ToOwned::to_owned),
                enabled: value
                    .get("enabled")
                    .and_then(|enabled| enabled.as_bool())
                    .unwrap_or(true),
            });
        }
    }

    if let Some(map) = json.get("mcp").and_then(|value| value.as_object()) {
        for (name, value) in map {
            let enabled = if let Some(enabled) = value.get("enabled").and_then(|enabled| enabled.as_bool()) {
                enabled
            } else if let Some(disabled) = value.get("disabled").and_then(|disabled| disabled.as_bool()) {
                !disabled
            } else {
                true
            };
            servers.push(McpServerRef {
                id: format!("{}:{}", agent.id, crate::discovery::slug(name)),
                name: name.clone(),
                source_agent: agent.id.clone(),
                source_path: Some(path.display().to_string()),
                command: value
                    .get("command")
                    .or_else(|| value.get("cmd"))
                    .and_then(|command| command.as_str())
                    .map(ToOwned::to_owned),
                enabled,
            });
        }
    }

    Ok(servers)
}

fn mcp_servers_from_toml(agent: &AgentProfile, path: &Path, content: &str) -> Result<Vec<McpServerRef>> {
    let value: toml::Value = toml::from_str(content).unwrap_or_else(|_| toml::Value::Table(Default::default()));
    let mut servers = Vec::new();
    if let Some(table) = value.get("mcp_servers").and_then(|value| value.as_table()) {
        for (name, value) in table {
            let command = value
                .get("command")
                .and_then(|command| command.as_str())
                .map(ToOwned::to_owned);
            servers.push(McpServerRef {
                id: format!("{}:{}", agent.id, crate::discovery::slug(name)),
                name: name.clone(),
                source_agent: agent.id.clone(),
                source_path: Some(path.display().to_string()),
                command,
                enabled: true,
            });
        }
    }
    Ok(servers)
}

fn provider_from_agent_id(agent_id: &str) -> Option<String> {
    match agent_id {
        "claude-code" => Some("Anthropic".to_string()),
        "gemini" => Some("Google".to_string()),
        "codex" => Some("OpenAI".to_string()),
        _ => None,
    }
}

fn dedupe_inventory(inventory: &mut CapabilityInventory) {
    let mut models = BTreeSet::new();
    inventory
        .models
        .retain(|model| models.insert(model.id.clone()));

    let mut skills = BTreeSet::new();
    inventory.skills.retain(|skill| skills.insert(skill.id.clone()));

    let mut servers = BTreeSet::new();
    inventory
        .mcp_servers
        .retain(|server| servers.insert(server.id.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::fake_agent_profile;

    #[tokio::test]
    async fn scans_fixture_capabilities() {
        let scanner = CapabilityScanner::new(".");
        let inventory = scanner.scan_agent(&fake_agent_profile()).await;
        assert!(inventory
            .models
            .iter()
            .any(|model| model.id == "fixture-small"));
        assert!(inventory
            .slash_commands
            .iter()
            .any(|command| command.name == "/review"));
    }
}
