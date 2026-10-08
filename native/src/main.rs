slint::include_modules!();

use codex_tool_core::discovery::{list_codex_sessions, list_default_workspace_files};
use slint::ComponentHandle;

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn refresh_data(ui: &AppWindow) {
    let mut had_error = false;
    let session_result = list_codex_sessions();
    let sessions = match session_result {
        Ok(rows) => rows,
        Err(error) => {
            had_error = true;
            ui.set_runtime_status("会话扫描失败".into());
            ui.set_workspace_root(format!("会话目录不可用 · {error}").into());
            Vec::new()
        }
    };
    let file_result = list_default_workspace_files();
    let files = match file_result {
        Ok(rows) => rows,
        Err(error) => {
            had_error = true;
            ui.set_runtime_status("文件扫描失败".into());
            ui.set_workspace_root(format!("文件目录不可用 · {error}").into());
            Vec::new()
        }
    };
    let bytes = files.iter().map(|file| file.size).sum::<u64>();

    ui.set_session_count(sessions.len().to_string().into());
    ui.set_file_count(files.len().to_string().into());
    ui.set_storage_size(format_bytes(bytes).into());
    if had_error {
        // Keep the concrete error context shown above instead of turning a
        // missing directory into a fake successful connection state.
    } else if !sessions.is_empty() || !files.is_empty() {
        ui.set_runtime_status("已读取本机 Codex 数据".into());
        ui.set_workspace_root("~/.codex · 本地扫描".into());
    } else {
        ui.set_runtime_status("已完成扫描，暂未发现 Codex 数据".into());
        ui.set_workspace_root("~/.codex · 空目录".into());
    }

    let session_preview = if sessions.is_empty() {
        "暂无本机会话\n运行 Codex 后，历史会话会显示在这里。".to_owned()
    } else {
        sessions
            .iter()
            .take(3)
            .map(|session| session.title.clone().unwrap_or_else(|| session.id.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    ui.set_session_preview(session_preview.into());

    let file_preview = if files.is_empty() {
        "暂无受管文件\n扫描 generated_images 和会话目录后，会显示实际占用。".to_owned()
    } else {
        files
            .iter()
            .take(3)
            .map(|file| format!("{} · {}", file.name, format_bytes(file.size)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    ui.set_file_preview(file_preview.into());
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    refresh_data(&ui);

    let weak = ui.as_weak();
    ui.on_refresh_data(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_data(&ui);
        }
    });

    let weak = ui.as_weak();
    ui.on_nav_clicked(move |page| {
        if let Some(ui) = weak.upgrade() {
            ui.set_active_page(page);
        }
    });

    ui.run()?;
    Ok(())
}
