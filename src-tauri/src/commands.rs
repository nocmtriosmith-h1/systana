//! Commands the UI calls through `invoke`.

use crate::collector::{Latest, Snapshot};
use crate::db;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub db: Mutex<Connection>,
    /// App name -> icon data URL (None = no icon found), filled on demand.
    pub icons: Mutex<std::collections::HashMap<String, Option<String>>>,
    pub db_path: PathBuf,
    pub latest: Latest,
    pub paused: Arc<AtomicBool>,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn get_latest(state: State<AppState>) -> Option<Snapshot> {
    state.latest.lock().ok().and_then(|l| l.clone())
}

#[tauri::command]
pub fn get_series(state: State<AppState>, from: i64, to: i64) -> CmdResult<db::Series> {
    let conn = state.db.lock().map_err(err)?;
    db::series(&conn, from, to).map_err(err)
}

#[tauri::command]
pub fn get_top_apps(state: State<AppState>, from: i64, to: i64, metric: String) -> CmdResult<Vec<db::TopApp>> {
    let conn = state.db.lock().map_err(err)?;
    db::top_apps(&conn, from, to, &metric).map_err(err)
}

/// Only `C:\...` style paths (optionally with the `\\?\` prefix). Network paths are refused:
/// reading an icon from `\\host\share` would make Windows connect to that host and send the user's credentials.
fn is_local_path(p: &str) -> bool {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
        || cfg!(not(windows)) && p.starts_with('/')
}

#[tauri::command]
pub fn get_app_icon(state: State<AppState>, name: String) -> Option<String> {
    if let Some(cached) = state.icons.lock().ok()?.get(&name) {
        return cached.clone();
    }
    let exe = state.db.lock().ok().and_then(|conn| db::app_exe(&conn, &name)).filter(|p| is_local_path(p));
    #[cfg(windows)]
    let icon = exe.and_then(|p| crate::icons::icon_data_url(std::path::Path::new(&p)));
    // Other platforms: letter tiles for now.
    #[cfg(not(windows))]
    let icon: Option<String> = exe.and(None);
    state.icons.lock().ok()?.insert(name, icon.clone());
    icon
}

#[tauri::command]
pub fn get_insights(state: State<AppState>, from: i64, to: i64) -> CmdResult<Vec<crate::insights::Insight>> {
    let conn = state.db.lock().map_err(err)?;
    crate::insights::detect(&conn, from, to).map_err(err)
}

/// Per-minute history of one app, for the app detail chart.
#[tauri::command]
pub fn get_app_history(state: State<AppState>, name: String, from: i64, to: i64) -> CmdResult<db::AppHistory> {
    let conn = state.db.lock().map_err(err)?;
    db::app_history(&conn, &name, from, to).map_err(err)
}

#[derive(Serialize)]
pub struct Settings {
    autostart: bool,
    retention_days: i64,
    paused: bool,
    language: Option<String>,
    db_bytes: u64,
    first_ts: Option<i64>,
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> CmdResult<Settings> {
    let conn = state.db.lock().map_err(err)?;
    // The WAL file holds recent writes, so count it too.
    let size = |p: PathBuf| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let wal = PathBuf::from(format!("{}-wal", state.db_path.display()));
    Ok(Settings {
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        retention_days: db::retention_days(&conn),
        paused: state.paused.load(Ordering::Relaxed),
        language: db::get_setting(&conn, "language"),
        db_bytes: size(state.db_path.clone()) + size(wal),
        first_ts: db::first_ts(&conn),
    })
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> CmdResult<()> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(err)
}

#[tauri::command]
pub fn set_retention_days(state: State<AppState>, days: i64) -> CmdResult<()> {
    let conn = state.db.lock().map_err(err)?;
    db::set_setting(&conn, "retention_days", &days.clamp(1, 3650).to_string()).map_err(err)
}

#[tauri::command]
pub fn set_paused(state: State<AppState>, paused: bool) {
    state.paused.store(paused, Ordering::Relaxed);
}

#[tauri::command]
pub fn set_language(state: State<AppState>, language: String) -> CmdResult<()> {
    if !matches!(language.as_str(), "vi" | "en") {
        return Err(format!("unsupported language: {language}"));
    }
    let conn = state.db.lock().map_err(err)?;
    db::set_setting(&conn, "language", &language).map_err(err)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn local_paths_only() {
        use super::is_local_path;
        assert!(is_local_path(r"C:\Program Files\App\app.exe"));
        assert!(is_local_path(r"\\?\C:\Windows\notepad.exe"));
        assert!(!is_local_path(r"\\attacker\share\x.exe"));
        assert!(!is_local_path(r"\\?\UNC\attacker\share\x.exe"));
        assert!(!is_local_path(r"\\.\pipe\x"));
        assert!(!is_local_path("relative.exe"));
    }
}
