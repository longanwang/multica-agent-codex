use multica_core::acp::AcpProcessClient;
use multica_core::types::AgentLaunch;
use std::collections::BTreeMap;

#[tokio::test]
async fn talks_to_fake_acp_agent_over_stdio() {
    let launch = AgentLaunch {
        command: "node".to_string(),
        args: vec!["tools/fake-acp-agent/index.mjs".to_string()],
        env: BTreeMap::new(),
        cwd: None,
    };

    let result = AcpProcessClient::run_prompt(&launch, "hello from test", None)
        .await
        .expect("fake ACP agent should respond");
    assert!(result.text.contains("hello from test"));
}

