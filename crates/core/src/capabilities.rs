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
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CapabilityScanner {
    home_dir: PathBuf,
    current_dir: PathBuf,
}

impl Default for CapabilityScanner {
    fn default() -> Self {
        Self {
            home_dir: std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

impl CapabilityScanner {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        let home_dir = home_dir.into();
        Self {
            current_dir: home_dir.clone(),
            home_dir,
        }
    }

    pub async fn scan_agent(&self, agent: &AgentProfile) -> CapabilityInventory {
        let mut inventory = CapabilityInventory::empty(agent.id.clone());
        inventory.auth = AuthState::Unknown;
        inventory.scanned_at = Utc::now();

        inventory.models.extend(self.models_from_known_agent(agent));
        inventory
            .models
            .extend(self.models_from_config(agent).unwrap_or_default());
        inventory
            .skills
            .extend(self.scan_skills(agent).unwrap_or_default());
        inventory
            .mcp_servers
            .extend(self.scan_mcp_servers(agent).unwrap_or_default());
        if let Ok(mcp_servers) = self.scan_opencode_mcp_cli(agent).await {
            inventory.mcp_servers.extend(mcp_servers);
        }
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

    fn models_from_config(&self, agent: &AgentProfile) -> Result<Vec<ModelOption>> {
        if agent.id.as_str() != "opencode" {
            return Ok(Vec::new());
        }

        let mut models = Vec::new();
        for path in self.opencode_config_paths() {
            if !path.exists() {
                continue;
            }
            let json = json_from_file(&path)?;
            if let Some(model) = json.get("model").and_then(Value::as_str) {
                models.push(ModelOption {
                    id: format!("opencode:{}", crate::discovery::slug(model)),
                    label: model.to_string(),
                    source_agent: agent.id.clone(),
                    provider: model
                        .split_once('/')
                        .map(|(provider, _)| provider.to_string())
                        .or_else(|| provider_from_agent_id(&agent.id)),
                });
            }
        }
        Ok(models)
    }

    fn scan_skills(&self, agent: &AgentProfile) -> Result<Vec<SkillRef>> {
        let patterns = match agent.id.as_str() {
            "codex" => vec![
                self.home_dir
                    .join(".codex")
                    .join("skills")
                    .join("*")
                    .join("SKILL.md"),
                self.home_dir
                    .join(".codex")
                    .join("plugins")
                    .join("cache")
                    .join("*")
                    .join("skills")
                    .join("*")
                    .join("SKILL.md"),
            ],
            "claude-code" => vec![
                self.home_dir
                    .join(".claude")
                    .join("skills")
                    .join("*")
                    .join("SKILL.md"),
                self.home_dir
                    .join(".claude")
                    .join("plugins")
                    .join("cache")
                    .join("*")
                    .join("*")
                    .join("*")
                    .join("skills")
                    .join("*")
                    .join("SKILL.md"),
                self.home_dir.join(".claude").join("commands").join("*.md"),
            ],
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
            "opencode" => vec![
                self.home_dir
                    .join(".config")
                    .join("opencode")
                    .join("skills")
                    .join("*")
                    .join("*.md"),
                self.home_dir
                    .join(".config")
                    .join("opencode")
                    .join("skills")
                    .join("*")
                    .join("SKILL.md"),
            ],
            "fixture-fast-reviewer" => vec![PathBuf::from(
                "crates/core/tests/fixtures/skills/*/SKILL.md",
            )],
            _ => Vec::new(),
        };

        let mut skills = Vec::new();
        for pattern in patterns {
            for entry in glob(&pattern.to_string_lossy())? {
                let path = entry?;
                skills.push(skill_from_path(agent, &path)?);
            }
        }
        if agent.id.as_str() == "opencode" {
            skills.extend(self.scan_opencode_plugins(agent).unwrap_or_default());
        }
        Ok(skills)
    }

    fn scan_mcp_servers(&self, agent: &AgentProfile) -> Result<Vec<McpServerRef>> {
        let patterns = match agent.id.as_str() {
            "codex" => vec![self.home_dir.join(".codex").join("config.toml")],
            "claude-code" => vec![
                self.current_dir.join(".mcp.json"),
                self.current_dir.join(".claude").join("settings.json"),
                self.current_dir.join(".claude").join("settings.local.json"),
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
                    .join(".config")
                    .join("opencode")
                    .join("opencode.jsonc"),
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

    fn scan_opencode_plugins(&self, agent: &AgentProfile) -> Result<Vec<SkillRef>> {
        let mut skills = Vec::new();
        for path in self.opencode_config_paths() {
            if !path.exists() {
                continue;
            }
            let json = json_from_file(&path)?;
            if let Some(plugins) = json.get("plugin").and_then(Value::as_array) {
                for plugin in plugins {
                    if let Some(name) = plugin_name(plugin) {
                        skills.push(SkillRef {
                            id: format!("{}:plugin:{}", agent.id, crate::discovery::slug(&name)),
                            name,
                            source_agent: agent.id.clone(),
                            source_path: Some(path.display().to_string()),
                            description: Some("OpenCode 插件".to_string()),
                        });
                    }
                }
            }
        }

        let package_path = self
            .home_dir
            .join(".config")
            .join("opencode")
            .join("package.json");
        if package_path.exists() {
            let json = json_from_file(&package_path)?;
            for section in ["dependencies", "devDependencies"] {
                if let Some(dependencies) = json.get(section).and_then(Value::as_object) {
                    for name in dependencies.keys() {
                        skills.push(SkillRef {
                            id: format!("{}:plugin:{}", agent.id, crate::discovery::slug(name)),
                            name: name.to_string(),
                            source_agent: agent.id.clone(),
                            source_path: Some(package_path.display().to_string()),
                            description: Some("OpenCode npm 插件依赖".to_string()),
                        });
                    }
                }
            }
        }

        Ok(skills)
    }

    fn opencode_config_paths(&self) -> Vec<PathBuf> {
        vec![
            self.home_dir
                .join(".config")
                .join("opencode")
                .join("opencode.json"),
            self.home_dir
                .join(".config")
                .join("opencode")
                .join("opencode.jsonc"),
            self.home_dir.join(".opencode.json"),
            self.home_dir.join("opencode.json"),
            self.home_dir
                .join("AppData")
                .join("Roaming")
                .join("opencode")
                .join("opencode.json"),
        ]
    }

    async fn scan_opencode_mcp_cli(&self, agent: &AgentProfile) -> Result<Vec<McpServerRef>> {
        if agent.id.as_str() != "opencode" {
            return Ok(Vec::new());
        }

        let args = vec!["mcp".to_string(), "list".to_string()];
        let output = tokio::time::timeout(
            Duration::from_secs(60),
            crate::process::command_with_args(&agent.launch.command, &args).output(),
        )
        .await
        .context("opencode mcp list timed out")?
        .with_context(|| format!("failed to run opencode MCP probe for {}", agent.name))?;
        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_opencode_mcp_list(agent, &stdout))
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
        .or_else(|| fallback_skill_name(path))
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

fn fallback_skill_name(path: &Path) -> Option<String> {
    if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
        return path
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().to_string());
    }
    path.file_stem()
        .map(|name| name.to_string_lossy().to_string())
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

fn json_from_file(path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON config {}", path.display()))?;
    parse_json_or_jsonc(&content)
}

fn parse_json_or_jsonc(content: &str) -> Result<Value> {
    serde_json::from_str(content)
        .or_else(|_| serde_json::from_str(&remove_trailing_commas(&strip_json_comments(content))))
        .context("failed to parse JSON/JSONC config")
}

fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            out.push(' ');
            continue;
        }

        out.push(ch);
    }

    out
}

fn remove_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    let mut in_string = false;
    let mut escape = false;

    while index < chars.len() {
        let ch = chars[index];
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            index += 1;
            continue;
        }

        if ch == ',' {
            let mut lookahead = index + 1;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead < chars.len() && matches!(chars[lookahead], '}' | ']') {
                index += 1;
                continue;
            }
        }

        out.push(ch);
        index += 1;
    }

    out
}

fn plugin_name(value: &Value) -> Option<String> {
    if let Some(name) = value.as_str() {
        return Some(readable_plugin_name(name));
    }
    value
        .get("name")
        .or_else(|| value.get("id"))
        .or_else(|| value.get("path"))
        .and_then(Value::as_str)
        .map(readable_plugin_name)
}

fn readable_plugin_name(value: &str) -> String {
    let path = Path::new(value);
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn mcp_servers_from_file(agent: &AgentProfile, path: &Path) -> Result<Vec<McpServerRef>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read MCP config {}", path.display()))?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
        return mcp_servers_from_toml(agent, path, &content);
    }
    let mut servers = Vec::new();
    let json = parse_json_or_jsonc(&content).unwrap_or_else(|_| serde_json::json!({}));
    collect_mcp_servers(agent, path, &json, &mut servers);
    Ok(servers)
}

fn collect_mcp_servers(
    agent: &AgentProfile,
    path: &Path,
    value: &Value,
    servers: &mut Vec<McpServerRef>,
) {
    if let Some(map) = value.as_object() {
        for (key, child) in map {
            if matches!(key.as_str(), "mcpServers" | "mcp") {
                if let Some(server_map) = child.as_object() {
                    push_mcp_servers_from_map(agent, path, server_map, servers);
                }
            }
            collect_mcp_servers(agent, path, child, servers);
        }
    }
    if let Some(array) = value.as_array() {
        for child in array {
            collect_mcp_servers(agent, path, child, servers);
        }
    }
}

fn push_mcp_servers_from_map(
    agent: &AgentProfile,
    path: &Path,
    map: &serde_json::Map<String, Value>,
    servers: &mut Vec<McpServerRef>,
) {
    for (name, value) in map {
        if !value.is_object() {
            continue;
        }
        let enabled =
            if let Some(enabled) = value.get("enabled").and_then(|enabled| enabled.as_bool()) {
                enabled
            } else if let Some(disabled) = value
                .get("disabled")
                .and_then(|disabled| disabled.as_bool())
            {
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

fn mcp_servers_from_toml(
    agent: &AgentProfile,
    path: &Path,
    content: &str,
) -> Result<Vec<McpServerRef>> {
    let value: toml::Value =
        toml::from_str(content).unwrap_or_else(|_| toml::Value::Table(Default::default()));
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

#[derive(Default)]
struct OpencodeMcpPendingServer {
    name: String,
    enabled: bool,
    command: Option<String>,
}

fn parse_opencode_mcp_list(agent: &AgentProfile, output: &str) -> Vec<McpServerRef> {
    let clean = strip_ansi(output);
    let mut servers = Vec::new();
    let mut pending: Option<OpencodeMcpPendingServer> = None;

    for raw_line in clean.lines() {
        let line = raw_line
            .trim()
            .trim_start_matches('|')
            .trim()
            .trim_start_matches('•')
            .trim();
        if line.is_empty() || line == "MCP Servers" || line.starts_with('—') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('✓').or_else(|| line.strip_prefix('○')) {
            if let Some(server) = pending.take() {
                servers.push(mcp_ref_from_pending(agent, server));
            }
            let enabled = line.starts_with('✓') && !line.contains("disabled");
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .to_string();
            pending = Some(OpencodeMcpPendingServer {
                name,
                enabled,
                command: None,
            });
            continue;
        }

        if let Some(server) = pending.as_mut() {
            if server.command.is_none() {
                server.command = Some(line.to_string());
            }
        }
    }

    if let Some(server) = pending {
        servers.push(mcp_ref_from_pending(agent, server));
    }

    servers
}

fn mcp_ref_from_pending(agent: &AgentProfile, server: OpencodeMcpPendingServer) -> McpServerRef {
    McpServerRef {
        id: format!("{}:{}", agent.id, crate::discovery::slug(&server.name)),
        name: server.name,
        source_agent: agent.id.clone(),
        source_path: Some("opencode mcp list".to_string()),
        command: server.command,
        enabled: server.enabled,
    }
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn provider_from_agent_id(agent_id: &str) -> Option<String> {
    match agent_id {
        "claude-code" => Some("Anthropic".to_string()),
        "gemini" => Some("Google".to_string()),
        "codex" => Some("OpenAI".to_string()),
        "opencode" => Some("OpenCode".to_string()),
        _ => None,
    }
}

fn dedupe_inventory(inventory: &mut CapabilityInventory) {
    let mut models = BTreeSet::new();
    inventory
        .models
        .retain(|model| models.insert(model.id.clone()));

    let mut skills = BTreeSet::new();
    inventory
        .skills
        .retain(|skill| skills.insert(skill.id.clone()));

    let mut servers = BTreeSet::new();
    inventory
        .mcp_servers
        .retain(|server| servers.insert(server.id.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{fake_agent_profile, manual_profile};

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

    #[tokio::test]
    async fn scans_claude_plugin_skills_and_nested_mcp() {
        let temp = tempfile::tempdir().unwrap();
        let skill_dir = temp
            .path()
            .join(".claude/plugins/cache/vendor/plugin/1.0.0/skills/review");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: chinese-review\n---\nReview code in Chinese.",
        )
        .unwrap();
        std::fs::write(
            temp.path().join(".claude.json"),
            r#"{
              "projects": {
                "D:/repo": {
                  "mcpServers": {
                    "tavily-remote-mcp": {
                      "command": "npx",
                      "enabled": true
                    }
                  }
                }
              }
            }"#,
        )
        .unwrap();

        let mut claude = manual_profile("Claude Code", "claude", vec![]);
        claude.id = "claude-code".to_string();
        let scanner = CapabilityScanner::new(temp.path());
        let inventory = scanner.scan_agent(&claude).await;

        assert!(inventory
            .skills
            .iter()
            .any(|skill| skill.name == "chinese-review"));
        assert!(inventory
            .mcp_servers
            .iter()
            .any(|server| server.name == "tavily-remote-mcp"));
    }

    #[tokio::test]
    async fn scans_opencode_jsonc_plugins_models_and_mcp() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join(".config/opencode");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("opencode.jsonc"),
            r#"{
              // JSONC comments are allowed by OpenCode.
              "model": "alibaba-cn/qwen3-coder-plus",
              "plugin": ["@opencode-ai/plugin"],
              "mcp": {
                "browser": {
                  "command": "npx",
                },
              },
            }"#,
        )
        .unwrap();
        std::fs::write(
            config_dir.join("package.json"),
            r#"{
              "dependencies": {
                "@opencode-ai/plugin": "latest"
              }
            }"#,
        )
        .unwrap();

        let mut opencode =
            manual_profile("OpenCode", "missing-opencode-for-test", vec!["acp".into()]);
        opencode.id = "opencode".to_string();
        let scanner = CapabilityScanner::new(temp.path());
        let inventory = scanner.scan_agent(&opencode).await;

        assert!(inventory
            .models
            .iter()
            .any(|model| model.label == "alibaba-cn/qwen3-coder-plus"));
        assert!(inventory
            .skills
            .iter()
            .any(|skill| skill.name == "@opencode-ai/plugin" || skill.name == "plugin"));
        assert!(inventory
            .mcp_servers
            .iter()
            .any(|server| server.name == "browser"));
    }

    #[test]
    fn parses_opencode_mcp_list_output() {
        let mut opencode = manual_profile("OpenCode", "opencode", vec!["acp".into()]);
        opencode.id = "opencode".to_string();
        let output = "\u{1b}[34m•\u{1b}[39m  ✓ websearch \u{1b}[90mconnected\n\
                      |      https://mcp.exa.ai/mcp?tools=web_search_exa\n\
                      •  ○ codegraph disabled\n\
                      |      codegraph serve --mcp\n\
                      —  2 server(s)";

        let servers = parse_opencode_mcp_list(&opencode, output);

        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].name, "websearch");
        assert!(servers[0].enabled);
        assert_eq!(
            servers[0].command.as_deref(),
            Some("https://mcp.exa.ai/mcp?tools=web_search_exa")
        );
        assert_eq!(servers[1].name, "codegraph");
        assert!(!servers[1].enabled);
    }
}
