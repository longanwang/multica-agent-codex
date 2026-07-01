# Multica Agent Codex

Multica is a local-first desktop shell for managing multiple ACP-compatible coding agents. The MVP includes:

- A Tauri v2 + React desktop console.
- A Rust core crate for ACP stdio clients, agent discovery, capability inventory, local permissions, task orchestration, and SQLite persistence.
- A lightweight Axum relay service for Feishu/WeCom bot ingress via outbound desktop WebSocket.
- Browser-safe mock data and fake agent paths so the UI can be previewed before real agents are installed.

## Repository Layout

```text
apps/desktop        Tauri + React UI
crates/core         Local control plane, ACP client, store, scanner, orchestrator
services/relay      Cloud relay for IM webhook ingress and desktop WebSocket delivery
tools/fake-acp-agent NDJSON ACP test agent for integration testing
```

## Local Development

```powershell
npm install
npm run dev
```

The frontend can run in a browser with mock data. A full desktop build also requires a Rust toolchain:

```powershell
cargo test --workspace
npm --workspace apps/desktop run tauri:dev
```

## MVP Boundaries

- Agent installation is not automated. Multica discovers known commands and lets users add manual launch profiles.
- Personal WeChat automation is intentionally out of scope. v1 targets Feishu Bot first and WeCom enterprise apps next.
- IM-triggered tasks enter a local approval queue by default.
- ACP agent-to-agent messages are mediated by the local orchestrator.

