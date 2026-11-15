mod commands;
mod model;
mod reminder;
mod state;
mod store;
mod tray;

use tauri::{Emitter, Manager};

pub fn run() {
    // Linux 原生 Wayland 下窗口无法定位/置顶，统一走 XWayland（WSLg 亦可）。
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
                let _ = app.emit("panel:open", ());
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::record_rep,
            commands::get_history,
            commands::update_settings,
            commands::set_paused,
            commands::snooze,
            commands::set_in_rep,
            commands::start_drag,
            commands::set_panel_open,
            commands::export_csv,
            commands::open_data_dir,
            commands::reset_data,
            commands::quit_app,
            commands::set_ball_visible,
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("failed to build tauri app")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                state::persist_position(app);
            }
        });
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;

    let store = store::Store::load(&data_dir);
    let warning = store.warning.clone();
    app.manage(state::AppRuntime::new(store, data_dir));

    // 首次启动（或历史数据里没有下次提醒时间）时初始化提醒倒计时
    {
        let runtime = app.state::<state::AppRuntime>();
        let mut store = match runtime.store.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if store.settings.next_due_ms.is_none() {
            let interval = store.settings.interval_ms();
            store.settings.next_due_ms = Some(model::now_ms() + interval);
            let _ = store.save_settings(&runtime.data_dir);
        }
    }

    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;

    let settings = state::rt(app.handle())
        .ok()
        .and_then(|runtime| runtime.store.lock().ok().map(|s| s.settings.clone()))
        .unwrap_or_default();

    let _ = win.set_always_on_top(settings.always_on_top);

    #[cfg(target_os = "macos")]
    {
        let _ = win.set_visible_on_all_workspaces(true);
    }

    let handle = app.handle().clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::Moved(position) = event {
            state::on_window_moved(&handle, position.x, position.y);
        }
    });

    // 确保窗口背景完全透明（否则部分平台会渲染出浅色方形底）
    let _ = win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));

    let _ = win.show();

    // 窗口创建阶段 outer_size 还是 0，等真正映射到屏幕上再定位。
    {
        let place_win = win.clone();
        let ball = settings.ball;
        let handle = app.handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(250));
            state::place_initial(&place_win, &ball);
            state::mark_ready(&handle);
        });
    }

    tray::create(app)?;
    reminder::spawn(app.handle().clone());

    // 拖动结束后吸附屏幕边缘
    {
        let handle = app.handle().clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(140));
            state::maybe_snap(&handle);
        });
    }

    if let Some(message) = warning {
        eprintln!("[tigang] {message}");
    }

    Ok(())
}
