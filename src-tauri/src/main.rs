#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use codex_tool_core::discovery::{self, CodexSessionRecord, QuarantineResult, WorkspaceFileRecord};
use codex_tool_core::{
    AccountPool, ArtifactRecord, ArtifactScanner, ProviderAccount, RoutePolicy, Router,
};
use std::path::Path;
use std::process::Command;
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::Emitter;

#[tauri::command]
fn health() -> serde_json::Value {
    serde_json::json!({ "ok": true, "service": "codex-nexus" })
}

#[tauri::command]
fn list_codex_sessions() -> Result<Vec<CodexSessionRecord>, String> {
    discovery::list_codex_sessions().map_err(|error| error.to_string())
}

#[tauri::command]
fn list_workspace_files(root: Option<String>) -> Result<Vec<WorkspaceFileRecord>, String> {
    match root.filter(|value| !value.trim().is_empty()) {
        Some(root) => discovery::list_workspace_files(root).map_err(|error| error.to_string()),
        None => discovery::list_default_workspace_files().map_err(|error| error.to_string()),
    }
}

#[tauri::command]
fn quarantine_path(path: String) -> Result<QuarantineResult, String> {
    discovery::quarantine_path(path).map_err(|error| error.to_string())
}

/// Reveal an existing local file or directory in the native file manager.
///
/// This deliberately uses an argument vector instead of a shell so paths
/// containing spaces or shell metacharacters are passed through unchanged.
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let requested = Path::new(path.trim());
    if !requested.is_absolute() {
        return Err("路径必须是绝对路径".to_owned());
    }
    if !requested.exists() {
        return Err(format!("路径不存在: {}", requested.display()));
    }
    let canonical = requested
        .canonicalize()
        .map_err(|error| format!("无法解析路径: {error}"))?;

    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open")
            .args(["-R"])
            .arg(&canonical)
            .spawn()
            .map_err(|error| format!("无法启动 Finder: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let argument = if canonical.is_file() {
            format!("/select,{}", canonical.display())
        } else {
            canonical.to_string_lossy().into_owned()
        };
        Command::new("explorer.exe")
            .arg(argument)
            .spawn()
            .map_err(|error| format!("无法启动文件资源管理器: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&canonical)
            .spawn()
            .map_err(|error| format!("无法启动文件管理器: {error}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("当前平台暂不支持打开文件管理器".to_owned())
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
        .setup(|app| {
            // Keep the application menu native.  On macOS this creates the
            // standard global menu bar; on Windows/Linux it becomes the
            // window menu.  The custom actions are forwarded to the React
            // shell through one typed event instead of relying on browser
            // keyboard handlers.
            let app_menu = SubmenuBuilder::new(app, "Codex Nexus")
                .about_with_text("关于 Codex Nexus", None)
                .separator()
                .quit_with_text("退出 Codex Nexus")
                .build()?;
            let workspace_menu = SubmenuBuilder::new(app, "工作区")
                .text("new_provider", "连接供应商…")
                .text("scan_files", "扫描 Codex 文件")
                .separator()
                .close_window_with_text("关闭窗口")
                .build()?;
            let edit_menu = SubmenuBuilder::new(app, "编辑")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;
            let view_menu = SubmenuBuilder::new(app, "显示")
                .minimize_with_text("最小化")
                .maximize_with_text("缩放")
                .fullscreen()
                .build()?;
            let menu = MenuBuilder::new(app)
                .item(&app_menu)
                .item(&workspace_menu)
                .item(&edit_menu)
                .item(&view_menu)
                .build()?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            if id == "new_provider" || id == "scan_files" {
                let _ = app.emit("nexus://menu", id);
            }
        })
        .invoke_handler(tauri::generate_handler![
            health,
            list_codex_sessions,
            list_workspace_files,
            quarantine_path,
            open_path,
            scan_artifacts,
            quarantine_artifact,
            restore_artifact,
            preview_route
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Nexus");
}
