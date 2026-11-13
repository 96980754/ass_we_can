use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::model::{
    self, local_date_of, now_ms, today_key, BallPos, Rep, Settings, SettingsPatch, Snapshot,
    MAX_REP_MS, MIN_VALID_MS,
};
use crate::store::Store;

pub struct AppRuntime {
    pub store: Mutex<Store>,
    pub in_rep: AtomicBool,
    pub panel_open: AtomicBool,
    pub last_pos_save: AtomicI64,
    /// 初始定位完成前不记录窗口移动，避免创建阶段的位置事件污染存档
    pub ready_at: AtomicI64,
    pub data_dir: PathBuf,
}

impl AppRuntime {
    pub fn new(store: Store, data_dir: PathBuf) -> Self {
        Self {
            store: Mutex::new(store),
            in_rep: AtomicBool::new(false),
            panel_open: AtomicBool::new(false),
            last_pos_save: AtomicI64::new(0),
            ready_at: AtomicI64::new(i64::MAX),
            data_dir,
        }
    }
}

pub fn rt(app: &AppHandle) -> Result<tauri::State<'_, AppRuntime>, String> {
    app.try_state::<AppRuntime>()
        .ok_or_else(|| "应用状态未初始化".to_string())
}

pub fn build_snapshot(runtime: &AppRuntime) -> Snapshot {
    let store = match runtime.store.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let today = today_key();
    let all_ms: u64 = store.reps.iter().map(|r| r.duration_ms).sum();
    let last_rep_ms = store
        .reps
        .iter()
        .map(|r| r.started_at_ms + r.duration_ms as i64)
        .max();

    Snapshot {
        settings: store.settings.clone(),
        today: model::day_stat(&store.reps, &today),
        all_count: store.reps.len() as u64,
        all_ms,
        streak_days: model::streak_days(&store.reps, &today),
        last7: model::last_n_days(&store.reps, &today, 7),
        last_rep_ms,
        data_dir: runtime.data_dir.display().to_string(),
        now_ms: now_ms(),
    }
}

pub fn snapshot(app: &AppHandle) -> Result<Snapshot, String> {
    let runtime = rt(app)?;
    Ok(build_snapshot(&runtime))
}

/// 状态变化后统一广播给前端与托盘。
pub fn publish(app: &AppHandle, snap: Snapshot) -> Snapshot {
    let _ = app.emit("state:changed", snap.clone());
    crate::tray::sync_labels(app, &snap);
    snap
}

pub fn record(app: &AppHandle, started_at_ms: i64, duration_ms: u64) -> Result<Snapshot, String> {
    if duration_ms <= MIN_VALID_MS {
        return Err("时长不足 1 秒，未计入".to_string());
    }
    let duration_ms = duration_ms.min(MAX_REP_MS);
    let runtime = rt(app)?;
    let now = now_ms();
    let (local_date, tz_offset_min) = local_date_of(started_at_ms.min(now));

    let rep = Rep {
        id: uuid::Uuid::new_v4().to_string(),
        started_at_ms,
        duration_ms,
        local_date,
        tz_offset_min,
    };

    let mut store = runtime
        .store
        .lock()
        .map_err(|_| "读取数据失败".to_string())?;
    store.reps.push(rep.clone());
    let interval = store.settings.interval_ms();
    store.settings.next_due_ms = Some(now + interval);
    if duration_ms >= store.settings.target_ms() {
        store.settings.pending_since_ms = None;
    }
    store
        .append_rep(&runtime.data_dir, &rep)
        .map_err(|e| e.to_string())?;
    store
        .save_settings(&runtime.data_dir)
        .map_err(|e| e.to_string())?;
    drop(store);

    Ok(build_snapshot(&runtime))
}

pub fn update_settings(app: &AppHandle, patch: SettingsPatch) -> Result<Snapshot, String> {
    let runtime = rt(app)?;
    let now = now_ms();
    let mut store = runtime
        .store
        .lock()
        .map_err(|_| "读取数据失败".to_string())?;

    let interval_changed = patch.interval_minutes.is_some();
    let paused_patch = patch.paused;

    {
        let settings = &mut store.settings;
        if let Some(v) = patch.interval_minutes {
            settings.interval_minutes = v.clamp(5, 240);
        }
        if let Some(v) = patch.target_seconds {
            settings.target_seconds = v.clamp(1, 60);
        }
        if let Some(v) = patch.min_valid_ms {
            settings.min_valid_ms = v.clamp(500, 5000);
        }
        if let Some(v) = patch.sound_enabled {
            settings.sound_enabled = v;
        }
        if let Some(v) = patch.opacity {
            settings.opacity = v.clamp(0.3, 1.0);
        }
        if let Some(v) = patch.always_on_top {
            settings.always_on_top = v;
        }
        if let Some(theme) = patch.theme {
            if matches!(theme.as_str(), "auto" | "light" | "dark") {
                settings.theme = theme;
            }
        }
    }

    if let Some(paused) = paused_patch {
        store.settings.paused = paused;
        if paused {
            store.settings.pending_since_ms = None;
        } else {
            let interval = store.settings.interval_ms();
            store.settings.next_due_ms = Some(now + interval);
        }
    }

    if interval_changed && !store.settings.paused {
        let interval = store.settings.interval_ms();
        store.settings.next_due_ms = Some(now + interval);
    }

    store
        .save_settings(&runtime.data_dir)
        .map_err(|e| e.to_string())?;
    drop(store);

    Ok(build_snapshot(&runtime))
}

pub fn set_paused(app: &AppHandle, paused: bool) -> Result<Snapshot, String> {
    let runtime = rt(app)?;
    let now = now_ms();
    let mut store = runtime
        .store
        .lock()
        .map_err(|_| "读取数据失败".to_string())?;
    store.settings.paused = paused;
    if paused {
        store.settings.pending_since_ms = None;
    } else {
        let interval = store.settings.interval_ms();
        store.settings.next_due_ms = Some(now + interval);
    }
    store
        .save_settings(&runtime.data_dir)
        .map_err(|e| e.to_string())?;
    drop(store);
    Ok(build_snapshot(&runtime))
}

pub fn snooze(app: &AppHandle, minutes: u32) -> Result<Snapshot, String> {
    let minutes = minutes.clamp(1, 240);
    let runtime = rt(app)?;
    let now = now_ms();
    let mut store = runtime
        .store
        .lock()
        .map_err(|_| "读取数据失败".to_string())?;
    store.settings.pending_since_ms = None;
    store.settings.next_due_ms = Some(now + minutes as i64 * 60_000);
    store
        .save_settings(&runtime.data_dir)
        .map_err(|e| e.to_string())?;
    drop(store);
    Ok(build_snapshot(&runtime))
}

pub fn clear_reps(app: &AppHandle) -> Result<Snapshot, String> {
    let runtime = rt(app)?;
    let now = now_ms();
    let mut store = runtime
        .store
        .lock()
        .map_err(|_| "读取数据失败".to_string())?;
    store.reps.clear();
    store
        .clear_reps_file(&runtime.data_dir)
        .map_err(|e| e.to_string())?;
    store.settings.pending_since_ms = None;
    let interval = store.settings.interval_ms();
    store.settings.next_due_ms = Some(now + interval);
    store
        .save_settings(&runtime.data_dir)
        .map_err(|e| e.to_string())?;
    drop(store);
    Ok(build_snapshot(&runtime))
}

pub fn set_in_rep(app: &AppHandle, in_rep: bool) {
    if let Ok(runtime) = rt(app) {
        runtime.in_rep.store(in_rep, Ordering::SeqCst);
    }
}

pub fn panel_open(app: &AppHandle, open: bool) {
    if let Ok(runtime) = rt(app) {
        runtime.panel_open.store(open, Ordering::SeqCst);
    }
}

pub fn place_initial(win: &tauri::WebviewWindow, ball: &BallPos) {
    let Ok(Some(monitor)) = win.current_monitor() else {
        return;
    };
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let win_size = win
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(140, 140));
    let margin = (24.0 * win.scale_factor().unwrap_or(1.0)).round() as i32;

    let (mut x, mut y) = match (ball.x, ball.y) {
        (Some(x), Some(y)) => (x, y),
        _ => (
            monitor_pos.x + monitor_size.width as i32 - win_size.width as i32 - margin,
            monitor_pos.y + (monitor_size.height as f64 * 0.22) as i32,
        ),
    };

    // 夹紧到当前显示器内，避免换显示器/分辨率变化后跑到屏幕外
    let max_x = (monitor_pos.x + monitor_size.width as i32 - win_size.width as i32)
        .max(monitor_pos.x);
    let max_y = (monitor_pos.y + monitor_size.height as i32 - win_size.height as i32)
        .max(monitor_pos.y);
    x = x.clamp(monitor_pos.x, max_x);
    y = y.clamp(monitor_pos.y, max_y);

    let _ = win.set_position(PhysicalPosition::new(x, y));
}

pub fn mark_ready(app: &AppHandle) {
    if let Ok(runtime) = rt(app) {
        runtime.ready_at.store(now_ms() + 1200, Ordering::SeqCst);
    }
}

pub fn on_window_moved(app: &AppHandle, x: i32, y: i32) {
    let Ok(runtime) = rt(app) else { return };
    if runtime.panel_open.load(Ordering::SeqCst) {
        return;
    }
    let now = now_ms();
    if now < runtime.ready_at.load(Ordering::SeqCst) {
        return;
    }
    if now - runtime.last_pos_save.load(Ordering::SeqCst) < 800 {
        return;
    }
    runtime.last_pos_save.store(now, Ordering::SeqCst);

    let mut store = match runtime.store.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if store.settings.ball.x == Some(x) && store.settings.ball.y == Some(y) {
        return;
    }
    store.settings.ball = BallPos {
        x: Some(x),
        y: Some(y),
    };
    let _ = store.save_settings(&runtime.data_dir);
}

pub fn persist_position(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(position) = win.outer_position() else {
        return;
    };
    if let Ok(runtime) = rt(app) {
        let mut store = match runtime.store.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        store.settings.ball = BallPos {
            x: Some(position.x),
            y: Some(position.y),
        };
        let _ = store.save_settings(&runtime.data_dir);
    }
}

// 供设置校验复用的默认值
#[allow(dead_code)]
pub fn default_settings() -> Settings {
    Settings::default()
}
