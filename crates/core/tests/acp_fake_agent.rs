use multica_core::acp::AcpProcessClient;
use multica_core::types::AgentLaunch;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[tokio::test]
async fn talks_to_fake_acp_agent_over_stdio() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("core crate should live under crates/core")
        .to_path_buf();
    let fake_agent = repo_root.join("tools").join("fake-acp-agent").join("index.mjs");
    let launch = AgentLaunch {
        command: "node".to_string(),
        args: vec![fake_agent.display().to_string()],
        env: BTreeMap::new(),
        cwd: None,
    };

    let result = AcpProcessClient::run_prompt(&launch, "hello from test", None)
        .await
        .expect("fake ACP agent should respond");
    assert!(result.text.contains("hello from test"));
}
