use multica_core::{
    AgentProfile, CapabilityInventory, DashboardSnapshot, MulticaCore, PermissionRequest,
    TaskSummary,
};
use serde_json::Value;
use std::sync::Arc;
use tauri::Manager;

#[derive(Clone)]
struct AppState {
    core: Arc<MulticaCore>,
}

#[tauri::command]
async fn dashboard_snapshot(state: tauri::State<'_, AppState>) -> Result<DashboardSnapshot, String> {
    state.core.dashboard_snapshot().map_err(to_command_error)
}

#[tauri::command]
async fn refresh_agents(state: tauri::State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    state.core.refresh_agents().await.map_err(to_command_error)
}

#[tauri::command]
async fn refresh_capabilities(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CapabilityInventory>, String> {
    state
        .core
        .refresh_capabilities()
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn create_task(
    state: tauri::State<'_, AppState>,
    title: String,
    prompt: String,
    agent_ids: Vec<String>,
) -> Result<TaskSummary, String> {
    state
        .core
        .create_task(title, prompt, agent_ids)
        .await
        .map_err(to_command_error)
}

#[tauri::command]
async fn add_manual_agent(
    state: tauri::State<'_, AppState>,
    name: String,
    command: String,
    args: Vec<String>,
) -> Result<AgentProfile, String> {
    state
        .core
        .add_manual_agent(name, command, args)
        .map_err(to_command_error)
}

#[tauri::command]
async fn ingest_connector_message(
    state: tauri::State<'_, AppState>,
    connector: String,
    tenant_id: String,
    conversation_id: String,
    sender_id: String,
    text: String,
    raw: Value,
) -> Result<PermissionRequest, String> {
    state
        .core
        .ingest_connector_message(connector, tenant_id, conversation_id, sender_id, text, raw)
        .map_err(to_command_error)
}

#[tauri::command]
async fn decide_permission(
    state: tauri::State<'_, AppState>,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    state
        .core
        .decide_permission(request_id, approved, "local-desktop")
        .map_err(to_command_error)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = match app.path().app_data_dir() {
                Ok(path) => path,
                Err(_) => std::env::current_dir()?.join(".multica-data"),
            };
            let core = Arc::new(MulticaCore::open(data_dir)?);
            let warm_core = core.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(agents) = warm_core.refresh_agents().await {
                    let _ = warm_core.refresh_capabilities().await;
                    tracing::info!(agent_count = agents.len(), "warmed Multica core");
                }
            });
            app.manage(AppState { core });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dashboard_snapshot,
            refresh_agents,
            refresh_capabilities,
            create_task,
            add_manual_agent,
            ingest_connector_message,
            decide_permission
        ])
        .run(tauri::generate_context!())
        .expect("error while running Multica desktop application");
}

fn to_command_error(error: anyhow::Error) -> String {
    error.to_string()
}
