use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::model::{DayHistory, SettingsPatch, Snapshot};
use crate::state;

const COMPACT: (f64, f64) = (140.0, 140.0);
const EXPANDED: (f64, f64) = (380.0, 560.0);

#[tauri::command]
pub fn get_snapshot(app: AppHandle) -> Result<Snapshot, String> {
    state::snapshot(&app)
}

#[tauri::command(rename_all = "camelCase")]
pub fn record_rep(app: AppHandle, started_at_ms: i64, duration_ms: u64) -> Result<Snapshot, String> {
    let snap = state::record(&app, started_at_ms, duration_ms)?;
    Ok(state::publish(&app, snap))
}

#[tauri::command]
pub fn get_history(app: AppHandle, days: Option<u32>) -> Result<Vec<DayHistory>, String> {
    let runtime = state::rt(&app)?;
    let store = runtime
        .store
        .lock()
        .map_err(|_| "读取数据失败".to_string())?;
    Ok(crate::model::history_days(
        &store.reps,
        days.unwrap_or(7).clamp(1, 365) as usize,
    ))
}

#[tauri::command]
pub fn update_settings(app: AppHandle, patch: SettingsPatch) -> Result<Snapshot, String> {
    let snap = state::update_settings(&app, patch)?;
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(snap.settings.always_on_top);
    }
    Ok(state::publish(&app, snap))
}

#[tauri::command]
pub fn set_paused(app: AppHandle, paused: bool) -> Result<Snapshot, String> {
    let snap = state::set_paused(&app, paused)?;
    Ok(state::publish(&app, snap))
}

#[tauri::command]
pub fn snooze(app: AppHandle, minutes: u32) -> Result<Snapshot, String> {
    let snap = state::snooze(&app, minutes)?;
    Ok(state::publish(&app, snap))
}

#[tauri::command(rename_all = "camelCase")]
pub fn set_in_rep(app: AppHandle, in_rep: bool) {
    state::set_in_rep(&app, in_rep);
}

#[tauri::command]
pub fn start_drag(app: AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    win.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_panel_open(app: AppHandle, open: bool) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    // 先标记状态，避免缩放引起的窗口移动被误存成悬浮球位置
    state::panel_open(&app, open);
    resize_panel(&win, open)?;
    if open {
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub async fn export_csv(app: AppHandle) -> Result<Option<String>, String> {
    let csv = {
        let runtime = state::rt(&app)?;
        let store = runtime
            .store
            .lock()
            .map_err(|_| "读取数据失败".to_string())?;
        crate::model::to_csv(&store.reps)
    };

    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_file_name("提肛记录.csv")
        .add_filter("CSV 文件", &["csv"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });

    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(|e| e.to_string())?;

    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;

    let mut content = String::from("\u{feff}");
    content.push_str(&csv);
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let dir = {
        let runtime = state::rt(&app)?;
        runtime.data_dir.display().to_string()
    };
    app.opener()
        .open_path(dir, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reset_data(app: AppHandle) -> Result<Snapshot, String> {
    let snap = state::clear_reps(&app)?;
    Ok(state::publish(&app, snap))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    state::persist_position(&app);
    app.exit(0);
}

#[tauri::command]
pub fn set_ball_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    if visible {
        win.show().map_err(|e| e.to_string())?;
    } else {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn resize_panel(win: &tauri::WebviewWindow, open: bool) -> Result<(), String> {
    let scale = win.scale_factor().map_err(|e| e.to_string())?;
    let (logical_w, logical_h) = if open { EXPANDED } else { COMPACT };
    let new_w = (logical_w * scale).round().max(1.0) as u32;
    let new_h = (logical_h * scale).round().max(1.0) as u32;

    let old_pos = win.outer_position().ok();
    let old_size = win.outer_size().ok();

    let size = PhysicalSize::new(new_w, new_h);
    if win.set_size(size).is_err() {
        // 某些平台在 resizable=false 时会忽略程序化尺寸调整
        let _ = win.set_resizable(true);
        win.set_size(size).map_err(|e| e.to_string())?;
        let _ = win.set_resizable(false);
    }

    // 以右下角为锚点缩放，让悬浮球停在原处
    if let (Some(pos), Some(old)) = (old_pos, old_size) {
        let mut x = pos.x + old.width as i32 - new_w as i32;
        let mut y = pos.y + old.height as i32 - new_h as i32;
        if let Ok(Some(monitor)) = win.current_monitor() {
            let mp = monitor.position();
            let ms = monitor.size();
            let max_x = (mp.x + ms.width as i32 - new_w as i32).max(mp.x);
            let max_y = (mp.y + ms.height as i32 - new_h as i32).max(mp.y);
            x = x.clamp(mp.x, max_x);
            y = y.clamp(mp.y, max_y);
        }
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    Ok(())
}
