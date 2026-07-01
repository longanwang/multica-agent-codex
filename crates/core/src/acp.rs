use crate::types::AgentLaunch;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

pub struct AcpProcessClient {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

#[derive(Debug, Clone)]
pub struct AcpPromptResult {
    pub session_id: Option<String>,
    pub text: String,
    pub raw: Value,
}

impl AcpProcessClient {
    pub async fn spawn(launch: &AgentLaunch) -> Result<Self> {
        let mut command = crate::process::command_with_stdio(&launch.command, &launch.args);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        if let Some(cwd) = &launch.cwd {
            command.current_dir(cwd);
        }
        for (key, value) in &launch.env {
            command.env(key, value);
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn ACP agent command {}", launch.command))?;
        let stdin = child.stdin.take().context("ACP agent stdin was not piped")?;
        let stdout = child.stdout.take().context("ACP agent stdout was not piped")?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 1,
        })
    }

    pub async fn initialize(&mut self) -> Result<Value> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientInfo": {
                    "name": "Multica",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "clientCapabilities": {
                    "terminal": true,
                    "fileSystem": true,
                    "permissions": "ask"
                }
            }),
        )
        .await
    }

    pub async fn new_session(
        &mut self,
        cwd: Option<String>,
        config: BTreeMap<String, Value>,
    ) -> Result<Value> {
        self.request(
            "session/new",
            json!({
                "cwd": cwd,
                "config": config,
                "mcpServers": []
            }),
        )
        .await
    }

    pub async fn prompt(&mut self, session_id: &str, prompt: &str) -> Result<AcpPromptResult> {
        let raw = self
            .request(
                "session/prompt",
                json!({
                    "sessionId": session_id,
                    "prompt": [
                        {"type": "text", "text": prompt}
                    ]
                }),
            )
            .await?;
        Ok(AcpPromptResult {
            session_id: Some(session_id.to_string()),
            text: extract_text(&raw),
            raw,
        })
    }

    pub async fn run_prompt(
        launch: &AgentLaunch,
        prompt: &str,
        cwd: Option<String>,
    ) -> Result<AcpPromptResult> {
        let mut client = Self::spawn(launch).await?;
        let _ = client.initialize().await?;
        let session = client.new_session(cwd, BTreeMap::new()).await?;
        let session_id = extract_session_id(&session)
            .context("ACP session/new response did not include a session id")?;
        let result = client.prompt(&session_id, prompt).await?;
        client.shutdown().await;
        Ok(result)
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let mut line = serde_json::to_vec(&request)?;
        line.push(b'\n');
        self.stdin.write_all(&line).await?;
        self.stdin.flush().await?;

        while let Some(line) = self.stdout.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(&line)
                .with_context(|| format!("ACP agent produced invalid JSON line: {line}"))?;
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = value.get("error") {
                    anyhow::bail!("ACP request {method} failed: {error}");
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            self.handle_agent_request(value).await?;
        }

        anyhow::bail!("ACP agent closed stdout while waiting for {method}")
    }

    async fn handle_agent_request(&mut self, value: Value) -> Result<()> {
        if value.get("method").is_some() {
            if let Some(id) = value.get("id").cloned() {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "outcome": "denied",
                        "message": "Multica MVP 要求智能体发起的客户端动作先经过桌面端明确审批。"
                    }
                });
                let mut line = serde_json::to_vec(&response)?;
                line.push(b'\n');
                self.stdin.write_all(&line).await?;
                self.stdin.flush().await?;
            }
        }
        Ok(())
    }

    pub async fn shutdown(mut self) {
        let _ = self.child.kill().await;
    }
}

fn extract_session_id(value: &Value) -> Option<String> {
    value
        .get("sessionId")
        .or_else(|| value.get("session_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extract_text(value: &Value) -> String {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        let mut parts = Vec::new();
        for item in content {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                parts.push(text.to_string());
            }
        }
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_text_from_common_shapes() {
        assert_eq!(extract_text(&json!({"text": "done"})), "done");
        assert_eq!(
            extract_text(&json!({"content": [{"text": "a"}, {"text": "b"}]})),
            "a\nb"
        );
    }
}
