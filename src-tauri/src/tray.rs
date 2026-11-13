use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::model::{format_short, Snapshot};
use crate::state;

pub struct TrayHandles {
    pub today: MenuItem<tauri::Wry>,
    pub pause: MenuItem<tauri::Wry>,
}

pub fn create(app: &tauri::App) -> tauri::Result<()> {
    let today = MenuItem::with_id(app, "today", "今日 0 次", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "打开面板", true, None::<&str>)?;
    let remind = MenuItem::with_id(app, "remind", "立即提醒", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "暂停提醒", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "隐藏 / 显示悬浮球", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&today, &open, &remind, &pause, &toggle, &quit],
    )?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("提肛")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_panel(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(TrayHandles { today, pause });
    Ok(())
}

pub fn sync_labels(app: &AppHandle, snapshot: &Snapshot) {
    if let Some(handles) = app.try_state::<TrayHandles>() {
        let _ = handles.today.set_text(format!(
            "今日 {} 次 · {}",
            snapshot.today.count,
            format_short(snapshot.today.total_ms)
        ));
        let _ = handles.pause.set_text(if snapshot.settings.paused {
            "恢复提醒"
        } else {
            "暂停提醒"
        });
    }
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_panel(app),
        "remind" => crate::reminder::fire_now(app),
        "pause" => {
            let paused = state::snapshot(app)
                .map(|s| s.settings.paused)
                .unwrap_or(false);
            if let Ok(snap) = state::set_paused(app, !paused) {
                state::publish(app, snap);
            }
        }
        "toggle" => toggle_window(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

fn show_panel(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit("panel:open", ());
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        match win.is_visible() {
            Ok(true) => {
                let _ = win.hide();
            }
            _ => {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
    }
}
