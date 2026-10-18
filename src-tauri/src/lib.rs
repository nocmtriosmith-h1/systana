mod collector;
mod commands;
mod counters;
mod db;
#[cfg(windows)]
mod icons;
mod insights;
#[cfg(windows)]
mod pdh;
#[cfg(windows)]
mod sensors;
mod temps;

/// Entry point for `systana.exe --sensors` (see sensors.rs).
#[cfg(windows)]
pub fn run_sensor_helper(once: bool) -> i32 {
    sensors::run_helper(once)
}

use commands::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WebviewWindowBuilder};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// Autostart launches with this flag so the app starts silently in the tray.
const MINIMIZED_ARG: &str = "--minimized";

/// The window (and its WebView2 processes, ~100 MB) only exists while it is open.
/// Closing it destroys it; the collector keeps running in the tray at a few MB.
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let Some(config) = app.config().app.windows.first() else { return };
    match WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()) {
        Ok(w) => {
            let _ = w.show();
            let _ = w.set_focus();
        }
        Err(e) => eprintln!("cannot open window: {e}"),
    }
}

/// Systana was called Sysrewind up to v0.1.1 (identifier `com.sysrewind.app`).
/// On the first start after the rename, move the old history over and drop the old
/// start-with-Windows entry, which points at an exe the installer has removed.
/// Returns true when old history was moved, so the caller re-enables start-with-Windows under the new name.
fn migrate_from_sysrewind(dir: &std::path::Path) -> bool {
    let Some(old) = dir.parent().map(|p| p.join("com.sysrewind.app")) else { return false };
    if dir.join("metrics.db").exists() || !old.join("metrics.db").exists() {
        return false;
    }
    for f in ["metrics.db", "metrics.db-wal", "metrics.db-shm"] {
        let (from, to) = (old.join(f), dir.join(f));
        if from.exists() && std::fs::rename(&from, &to).is_err() {
            let _ = std::fs::copy(&from, &to);
        }
    }
    let _ = std::fs::remove_dir(&old); // only succeeds when empty
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let system32 = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| r"C:\Windows".into()).join("System32");
        let _ = std::process::Command::new(system32.join("reg.exe"))
            .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Sysrewind", "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Opening the app a second time just brings the existing window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED_ARG])))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("metrics.db");
            let migrated = migrate_from_sysrewind(&dir);
            let conn = db::open(&db_path)?;

            // Zero config: turn on start-with-Windows the first time the app runs.
            // Dev builds skip this so they never register target\debug\*.exe.
            if !cfg!(debug_assertions) {
                let autolaunch = app.autolaunch();
                if migrated || db::get_setting(&conn, "autostart_initialized").is_none() {
                    let _ = autolaunch.enable();
                    db::set_setting(&conn, "autostart_initialized", "1")?;
                } else if autolaunch.is_enabled().unwrap_or(false) {
                    // Re-register so the entry follows this exe (e.g. after moving from a portable copy to Program Files).
                    let _ = autolaunch.enable();
                }
            }

            let latest = Arc::new(Mutex::new(None));
            let paused = Arc::new(AtomicBool::new(false));
            collector::spawn(db_path.clone(), latest.clone(), paused.clone());
            app.manage(AppState { db: Mutex::new(conn), icons: Mutex::default(), db_path, latest, paused });

            let open = MenuItem::with_id(app, "open", "Mở Systana / Open", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Thoát / Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main")
                .tooltip("Systana")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            if !std::env::args().any(|a| a == MINIMIZED_ARG) {
                show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_latest,
            commands::get_series,
            commands::get_top_apps,
            commands::get_insights,
            commands::get_app_icon,
            commands::get_app_history,
            commands::get_settings,
            commands::set_autostart,
            commands::set_retention_days,
            commands::set_paused,
            commands::set_language,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Systana")
        .run(|_app, event| {
            // Closing the last window keeps recording in the tray.
            // Only "Quit" in the tray menu (app.exit, which sets a code) really exits.
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
