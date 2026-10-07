#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use codex_tool_core::{
    AccountPool, ArtifactRecord, ArtifactScanner, ProviderAccount, RoutePolicy, Router,
};

#[tauri::command]
fn health() -> serde_json::Value {
    serde_json::json!({ "ok": true, "service": "codex-nexus" })
}

#[tauri::command]
fn scan_artifacts(
    root: String,
    job_id: String,
    now_ms: u64,
) -> Result<Vec<ArtifactRecord>, String> {
    ArtifactScanner::new(root)
        .map_err(|error| error.to_string())?
        .scan_job(&job_id, now_ms)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn quarantine_artifact(root: String, record: ArtifactRecord) -> Result<ArtifactRecord, String> {
    ArtifactScanner::new(root)
        .map_err(|error| error.to_string())?
        .quarantine_record(&record)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_artifact(root: String, record: ArtifactRecord) -> Result<ArtifactRecord, String> {
    ArtifactScanner::new(root)
        .map_err(|error| error.to_string())?
        .restore(&record)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_route(
    accounts: Vec<ProviderAccount>,
    policy: RoutePolicy,
    session_id: Option<String>,
    now_ms: u64,
) -> Result<String, String> {
    let mut pool = AccountPool::new();
    for account in accounts {
        pool.insert(account).map_err(|error| error.to_string())?;
    }
    Router::new(pool)
        .select(session_id.as_deref(), &policy, now_ms)
        .map(|decision| decision.account_id)
        .map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            health,
            scan_artifacts,
            quarantine_artifact,
            restore_artifact,
            preview_route
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Nexus");
}
