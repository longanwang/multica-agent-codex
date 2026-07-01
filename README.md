# Multica Agent Codex

Multica 是一个本地优先的桌面壳，用来统一管理多个兼容 ACP 的编码智能体。当前 MVP 包含：

- Tauri v2 + React 桌面控制台。
- Rust 核心 crate：负责 ACP stdio client、智能体发现、能力清单、本机审批、任务编排和 SQLite 持久化。
- 轻量 Axum 中继服务：用于飞书/企业微信 bot 入口，并通过桌面端出站 WebSocket 投递消息。
- 浏览器安全的 mock 数据和模拟智能体路径：即使本机还没有安装真实智能体，也可以预览界面和主流程。

## 仓库结构

```text
apps/desktop         Tauri + React 桌面界面
crates/core          本地控制平面、ACP client、存储、扫描器、编排器
services/relay       IM webhook 入口和桌面 WebSocket 投递中继
tools/fake-acp-agent NDJSON ACP 测试智能体
```

## 本地开发

```powershell
npm install
npm run dev
```

前端可以直接在浏览器中使用 mock 数据运行。完整桌面构建还需要 Rust 工具链：

```powershell
cargo test --workspace
npm run tauri:dev
```

## 真实调用本地 Agent

`npm run dev` 启动的是浏览器预览，只能跑 mock 数据，不能启动本地进程。要真实调用本机 agent，需要用 Tauri 桌面模式：

```powershell
npm install
npm run tauri:dev
```

桌面端启动后，进入“智能体”工作区并刷新控制台。Multica 会从 ACP Registry、Windows `PATH` 和常见安装位置发现 `opencode`、`claude`、`gemini`、`codex` 等命令。未自动发现的 agent 可以在“智能体列表”中手动添加名称、启动命令和参数。

手动添加时，命令必须是兼容 ACP stdio 的启动入口。普通交互式 CLI 如果没有进入 ACP server/client 协议模式，任务会在 `initialize`、`session/new` 或 `session/prompt` 阶段失败。命令路径可以写成 `claude`、`codex` 这类 PATH 命令，也可以写成本机绝对路径；参数需要按对应 agent 的 ACP 启动方式单独填写。

Windows 上如果还没有 Rust/Tauri 构建环境，可以先安装 Rust：

```powershell
winget install Rustlang.Rustup
rustup update stable
```

## MVP 边界

- 不自动安装智能体。Multica 只发现已知命令，并允许用户手动添加启动配置。
- 不支持个人微信自动化。v1 优先接入飞书 Bot，随后接入企业微信应用。
- IM 触发的任务默认进入本机审批队列。
- ACP 智能体之间的消息由本地编排器中转，不让智能体直接互调。
