use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::model::now_ms;
use crate::state::{rt, snapshot, AppRuntime};

/// 后台常驻提醒线程：按墙钟时间判断，睡眠/休眠唤醒后能自愈；
/// 窗口隐藏或在托盘时依然可靠。
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(800));
        tick(&app);
    });
}

fn tick(app: &AppHandle) {
    let Ok(runtime) = rt(app) else { return };
    if runtime.in_rep.load(Ordering::SeqCst) {
        return;
    }
    let now = now_ms();
    if !mark_due(&runtime, now) {
        return;
    }
    fire(app, now);
}

/// 到点则推进下一次时间并标记待完成；返回本次是否触发。
fn mark_due(runtime: &AppRuntime, now: i64) -> bool {
    let Ok(mut store) = runtime.store.lock() else {
        return false;
    };
    if store.settings.paused {
        return false;
    }
    let Some(due) = store.settings.next_due_ms else {
        return false;
    };
    if now < due {
        return false;
    }
    let interval = store.settings.interval_ms();
    store.settings.next_due_ms = Some(now + interval);
    store.settings.pending_since_ms = Some(now);
    let _ = store.save_settings(&runtime.data_dir);
    true
}

fn fire(app: &AppHandle, at: i64) {
    let _ = app.emit("reminder:due", serde_json::json!({ "at": at }));

    let target = snapshot(app)
        .map(|s| s.settings.target_seconds)
        .unwrap_or(5);
    let visible = app
        .get_webview_window("main")
        .and_then(|win| win.is_visible().ok())
        .unwrap_or(false);

    if !visible {
        notify(app, target);
    }

    if let Ok(snap) = snapshot(app) {
        let _ = app.emit("state:changed", snap);
    }
}

fn notify(app: &AppHandle, target_seconds: u32) {
    let _ = app
        .notification()
        .builder()
        .title("该提肛了")
        .body(format!("按住悬浮球，坚持 {target_seconds} 秒"))
        .show();
}

/// 托盘「立即提醒」：无视当前倒计时，立即触发一次。
pub fn fire_now(app: &AppHandle) {
    let now = now_ms();
    if let Ok(runtime) = rt(app) {
        if let Ok(mut store) = runtime.store.lock() {
            store.settings.pending_since_ms = Some(now);
            let interval = store.settings.interval_ms();
            store.settings.next_due_ms = Some(now + interval);
            let _ = store.save_settings(&runtime.data_dir);
        }
    }
    fire(app, now);
}
