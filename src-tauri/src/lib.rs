mod agenda;
mod campus;
mod client;
mod clock;
mod course;
mod crypto;
mod error;
mod hotkey;
mod ipgw;
mod models;
mod periods;
mod util;
mod webvpn;
mod desk_drop;
mod share;
mod wlan;

use client::{
    aggregate_month, load_month_cache, month_cache_fresh, save_month_cache, NeuClient, SessionState,
};
use course::{
    apply_semester, fetched_local_date, load_semester, save_semester, semester_cache_stale,
    should_fetch_courses,
};

fn week1_opt(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}
use error::Result;
use models::*;
use chrono::{Datelike, NaiveDate, Timelike};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewWindow};

const SCHOOL_DATA_UNAVAILABLE: &str = "无锡学院数据接口尚未接入；本版本不会连接东北大学服务。";

fn clear_school_snapshot(snapshot: &mut Snapshot) {
    snapshot.fetched_at.clear();
    snapshot.network = "unknown".into();
    snapshot.logged_in = false;
    snapshot.username = None;
    snapshot.card = None;
    snapshot.card_bills = None;
    snapshot.card_spend = None;
    snapshot.card_month = None;
    snapshot.bills_truncated = None;
    snapshot.net = None;
    snapshot.elec = None;
    snapshot.courses_today = None;
    snapshot.semester = None;
    snapshot.teaching_week = None;
    snapshot.offline = false;
    snapshot.errors = SnapshotErrors::default();
}
use tokio::sync::Mutex as AsyncMutex;

pub struct AppState {
    client: AsyncMutex<Option<Arc<NeuClient>>>,
    epoch: AtomicU64,
    settings: Mutex<AppSettings>,
    snapshot: Mutex<Snapshot>,
    pending_login: Mutex<Option<PendingLogin>>,
    pause_bills: AtomicBool,
    desk_open_lock: Mutex<()>,
    /// 刷新串行。避免定时任务和手动刷新同时写 snapshot。
    refresh_gate: AsyncMutex<()>,
    /// 自动课表同步今天已经打过教务（不论成败）。手动刷新不受这一天限制。
    course_attempt_day: Mutex<Option<NaiveDate>>,
    data_dir: PathBuf,
    tray_balances: Mutex<Option<TrayBalances>>,
}

struct TrayBalances {
    card: MenuItem<tauri::Wry>,
    net: MenuItem<tauri::Wry>,
    elec: MenuItem<tauri::Wry>,
}

/// 23:00–06:00 电费系统经常连不上。这段时间失败就留缓存，不标同步失败。
fn elec_night(hour: u32) -> bool {
    hour >= 23 || hour < 6
}

fn normalize_room(raw: &str) -> String {
    raw.trim().replace(['-', ' '], "")
}

/// 失败时怎么处理电费缓存。`quiet` 必须用处理结果那一刻的钟，不要用刷新开始时的钟。
fn settle_elec(
    elec: &mut Option<ElecInfo>,
    error: &mut Option<String>,
    want_room: &str,
    quiet: bool,
    err: &str,
) {
    let want = normalize_room(want_room);
    let cached = elec
        .as_ref()
        .and_then(|old| old.room_no.as_deref())
        .map(normalize_room)
        .unwrap_or_default();
    let room_changed = !cached.is_empty() && !want.is_empty() && cached != want;
    let usable = elec
        .as_ref()
        .is_some_and(|old| !old.unbound && old.remain_kwh.is_some())
        && !room_changed;
    if room_changed {
        *elec = None;
    }
    if quiet && usable {
        if let Some(old) = elec.as_mut() {
            old.stale = true;
            if old
                .room_no
                .as_deref()
                .map(normalize_room)
                .unwrap_or_default()
                .is_empty()
                && !want.is_empty()
            {
                old.room_no = Some(want);
            }
        }
        *error = None;
    } else {
        if let Some(old) = elec.as_mut() {
            old.stale = false;
        }
        *error = Some(err.to_string());
    }
}

async fn ensure_client(state: &AppState) -> Result<Arc<NeuClient>> {
    let mut g = state.client.lock().await;
    if g.is_none() {
        *g = Some(Arc::new(NeuClient::new(state.data_dir.clone())?));
    }
    Ok(g.as_ref().unwrap().clone())
}

async fn live_client(state: &AppState) -> Result<Arc<NeuClient>> {
    state
        .client
        .lock()
        .await
        .clone()
        .ok_or_else(|| "未登录".into())
}

async fn with_timeout<T>(secs: u64, fut: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(Duration::from_secs(secs), fut)
        .await
        .unwrap_or_else(|_| Err("同步超时，请再刷新一次。".into()))
}

struct PendingLogin {
    username: String,
    password: String,
    remember: bool,
    auto: bool,
}

fn load_settings(dir: &PathBuf) -> AppSettings {
    let p = dir.join("settings.json");
    let mut s: AppSettings = fs::read(&p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if s.reveal_hotkey.eq_ignore_ascii_case("tab+q") {
        s.reveal_hotkey = "Ctrl+Q".into();
        save_settings_file(dir, &s);
    }
    s.jwxt_password_saved = !s.jwxt_username.trim().is_empty()
        && jwxt_keyring_get(&s.jwxt_username).is_some();
    s
}

fn save_settings_file(dir: &PathBuf, s: &AppSettings) {
    let _ = fs::write(dir.join("settings.json"), serde_json::to_vec_pretty(s).unwrap_or_default());
}

fn save_snapshot_file(dir: &PathBuf, s: &Snapshot) {
    let mut disk = s.clone();
    disk.offline = false;
    disk.agenda = Vec::new();
    let _ = fs::write(
        dir.join("snapshot.json"),
        serde_json::to_vec_pretty(&disk).unwrap_or_default(),
    );
}

fn keyring_set(user: &str, pass: &str) -> Result<()> {
    let e = keyring::Entry::new("com.sapphirexu.wuxicampusdashboard", user).map_err(|e| e.to_string())?;
    e.set_password(pass).map_err(|e| e.to_string())?;
    Ok(())
}

fn keyring_get(user: &str) -> Option<String> {
    keyring::Entry::new("com.sapphirexu.wuxicampusdashboard", user)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.is_empty())
}

fn keyring_delete(user: &str) {
    if let Ok(e) = keyring::Entry::new("com.sapphirexu.wuxicampusdashboard", user) {
        let _ = e.delete_credential();
    }
}

const JWXT_CREDENTIAL_SERVICE: &str = "com.sapphirexu.wuxicampusdashboard.jwxt";

fn jwxt_keyring_set(user: &str, pass: &str) -> std::result::Result<(), String> {
    let entry = keyring::Entry::new(JWXT_CREDENTIAL_SERVICE, user).map_err(|e| e.to_string())?;
    entry.set_password(pass).map_err(|e| e.to_string())
}

fn jwxt_keyring_get(user: &str) -> Option<String> {
    keyring::Entry::new(JWXT_CREDENTIAL_SERVICE, user)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .filter(|password| !password.is_empty())
}

fn jwxt_keyring_delete(user: &str) {
    if let Ok(entry) = keyring::Entry::new(JWXT_CREDENTIAL_SERVICE, user) {
        let _ = entry.delete_credential();
    }
}

pub(crate) fn stored_login(state: &AppState) -> Option<(String, String)> {
    let s = state.settings.lock().unwrap();
    if !s.remember_password || s.last_username.trim().is_empty() {
        return None;
    }
    let user = s.last_username.clone();
    drop(s);
    let pass = keyring_get(&user)?;
    Some((user, pass))
}

fn tray_labels(snap: &Snapshot) -> (String, String, String) {
    let _ = snap;
    let card = "校园卡数据待接入".into();
    let net = "校园网络数据待接入".into();
    let elec = "宿舍服务数据待接入".into();
    (card, net, elec)
}

fn sync_tray(app: &AppHandle, snap: &Snapshot) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let guard = state.tray_balances.lock().unwrap();
    let Some(t) = guard.as_ref() else {
        return;
    };
    let (card, net, elec) = tray_labels(snap);
    let _ = t.card.set_text(card);
    let _ = t.net.set_text(net);
    let _ = t.elec.set_text(elec);
}

fn emit_snapshot(app: &AppHandle, snap: &Snapshot) {
    sync_tray(app, snap);
    let _ = app.emit("snapshot-updated", snap);
}

fn apply_autostart(enable: bool) -> Result<()> {
    if cfg!(debug_assertions) {
        let _ = enable;
        return Ok(());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let exe = std::env::current_exe()?;
        let exe_s = exe.to_string_lossy().replace('/', "\\");
        if enable {
            let quoted = format!("\"{exe_s}\"");
            let st = std::process::Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "Nidong",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &quoted,
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status()
                .map_err(|e| e.to_string())?;
            if !st.success() {
                return Err("无法写入开机自启".into());
            }
        } else {
            let _ = std::process::Command::new("reg")
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "Nidong",
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }
    let _ = enable;
    Ok(())
}

fn island_logical_width(win: &WebviewWindow) -> f64 {
    let scale = win.scale_factor().unwrap_or(1.0);
    win.outer_size()
        .ok()
        .map(|s| s.width as f64 / scale)
        .filter(|w| *w > 1.0)
        .unwrap_or(420.0)
}

fn clamp_pos(win: &WebviewWindow, x: f64, y: f64) -> (f64, f64) {
    let mut x = x;
    // 旧默认距顶 8px；贴顶时上沿不应留空。
    let mut y = if y <= 8.0 { 0.0 } else { y };
    let ww = island_logical_width(win);
    if let Ok(Some(m)) = win.current_monitor() {
        let scale = m.scale_factor();
        let origin = m.position();
        let size = m.size();
        let mx = origin.x as f64 / scale;
        let my = origin.y as f64 / scale;
        let sw = size.width as f64 / scale;
        let sh = size.height as f64 / scale;
        x = x.clamp(mx, (mx + sw - ww).max(mx));
        y = y.clamp(my, (my + sh - 40.0).max(my));
        if y <= my + 8.0 {
            y = my;
        }
    }
    (x, y)
}

fn place_window(win: &WebviewWindow, settings: &AppSettings) {
    if let (Some(x), Some(y)) = (settings.island_x, settings.island_y) {
        let (x, y) = clamp_pos(win, x, y);
        let _ = win.set_position(LogicalPosition::new(x, y));
        return;
    }
    if let Ok(Some(m)) = win.current_monitor() {
        let size = m.size();
        let scale = m.scale_factor();
        let sw = size.width as f64 / scale;
        let x = ((sw - 420.0) / 2.0).max(0.0);
        let _ = win.set_position(LogicalPosition::new(x, 0.0));
    }
}

fn wipe_local_data(dir: &PathBuf, keep_agenda: bool) {
    let _ = fs::remove_file(dir.join("cookies.json"));
    let _ = fs::remove_file(dir.join("snapshot.json"));
    if !keep_agenda {
        let _ = fs::remove_file(dir.join("agenda.json"));
    }
    clear_session_open(dir);
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name();
            let s = name.to_string_lossy();
            if s.starts_with("bills-") && s.ends_with(".json") {
                let _ = fs::remove_file(e.path());
            }
            if s == "semester.json" {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

const SESSION_FLAG: &str = "session.open";

fn session_is_open(dir: &PathBuf) -> bool {
    dir.join(SESSION_FLAG).is_file()
}

fn mark_session_open(dir: &PathBuf) {
    let _ = fs::write(dir.join(SESSION_FLAG), b"1");
}

fn clear_session_open(dir: &PathBuf) {
    let _ = fs::remove_file(dir.join(SESSION_FLAG));
}

/// 旧版没有 session.open：有学号缓存且还留着 cookie，视为没点过退出登录。
fn migrate_session_flag(dir: &PathBuf, snap: &Snapshot) {
    if session_is_open(dir) {
        return;
    }
    let has_user = snap
        .username
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if has_user && dir.join("cookies.json").is_file() {
        mark_session_open(dir);
    }
}

fn attach_local(dir: &PathBuf, snap: &mut Snapshot) {
    snap.agenda = agenda::load(dir);
}

fn apply_offline(state: &AppState) -> Snapshot {
    let mut snap = state.snapshot.lock().unwrap().clone();
    snap.offline = true;
    snap.errors.auth = None;
    if session_is_open(&state.data_dir) {
        snap.logged_in = true;
    }
    attach_local(&state.data_dir, &mut snap);
    *state.snapshot.lock().unwrap() = snap.clone();
    snap
}

#[tauri::command]
fn bootstrap(state: State<AppState>) -> Snapshot {
    let mut snap = state.snapshot.lock().unwrap().clone();
    clear_school_snapshot(&mut snap);
    attach_local(&state.data_dir, &mut snap);
    *state.snapshot.lock().unwrap() = snap.clone();
    snap
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ClockNow {
    iso: String,
    overridden: bool,
}

/// 返回泥洞认定的“当前时间”。无 dev-now.txt / NIDONG_NOW 时即系统时间。
#[tauri::command]
fn clock_now() -> ClockNow {
    ClockNow {
        iso: crate::clock::now().to_rfc3339(),
        overridden: crate::clock::is_overridden(),
    }
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_local_profile(
    app: AppHandle,
    state: State<AppState>,
    display_name: String,
    jwxt_username: String,
    jwxt_password: Option<String>,
) -> Result<AppSettings> {
    let display_name = display_name.trim().to_string();
    if display_name.is_empty() {
        return Err("请先填写昵称".into());
    }
    if display_name.chars().count() > 24 {
        return Err("昵称不能超过 24 个字符".into());
    }

    let username = jwxt_username.trim().to_string();
    let password = jwxt_password.filter(|value| !value.is_empty());
    let old_username = state.settings.lock().unwrap().jwxt_username.clone();

    if username.is_empty() && password.is_some() {
        return Err("填写教务密码前，请先填写教务用户名".into());
    }

    if username.is_empty() {
        if !old_username.is_empty() {
            jwxt_keyring_delete(&old_username);
        }
    } else if let Some(password) = password.as_deref() {
        jwxt_keyring_set(&username, password)
            .map_err(|_| "无法将密码保存到本机系统凭据库，请检查系统凭据服务".to_string())?;
        if !old_username.is_empty() && old_username != username {
            jwxt_keyring_delete(&old_username);
        }
    } else if jwxt_keyring_get(&username).is_none() {
        return Err("请输入该教务账号的密码，或清空账号后稍后再设置".into());
    }

    let password_saved = !username.is_empty() && jwxt_keyring_get(&username).is_some();
    let settings = {
        let mut current = state.settings.lock().unwrap();
        current.display_name = display_name;
        current.profile_setup_complete = true;
        current.jwxt_username = username;
        current.jwxt_password_saved = password_saved;
        current.clone()
    };
    save_settings_file(&state.data_dir, &settings);
    let _ = app.emit("settings-updated", &settings);
    Ok(settings)
}

#[tauri::command]
fn clear_local_jwxt_credentials(app: AppHandle, state: State<AppState>) -> Result<AppSettings> {
    let settings = {
        let mut current = state.settings.lock().unwrap();
        if !current.jwxt_username.is_empty() {
            jwxt_keyring_delete(&current.jwxt_username);
        }
        current.jwxt_username.clear();
        current.jwxt_password_saved = false;
        current.clone()
    };
    save_settings_file(&state.data_dir, &settings);
    let _ = app.emit("settings-updated", &settings);
    Ok(settings)
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<AppState>, mut settings: AppSettings) -> Result<AppSettings> {
    {
        let mut g = state.settings.lock().unwrap();
        settings.island_x = g.island_x;
        settings.island_y = g.island_y;
        settings.display_name = g.display_name.clone();
        settings.profile_setup_complete = g.profile_setup_complete;
        settings.jwxt_username = g.jwxt_username.clone();
        settings.jwxt_password_saved = g.jwxt_password_saved;
        if settings.last_username.is_empty() {
            settings.last_username = g.last_username.clone();
        }
        if !settings.remember_password {
            settings.auto_login = false;
            let extra = state.snapshot.lock().unwrap().username.clone();
            if !settings.last_username.is_empty() {
                keyring_delete(&settings.last_username);
            }
            if let Some(u) = extra {
                if !u.is_empty() && u != settings.last_username {
                    keyring_delete(&u);
                }
            }
        }
        crate::hotkey::update(&settings);
        apply_autostart(settings.autostart)?;
        *g = settings.clone();
    }
    save_settings_file(&state.data_dir, &settings);
    let _ = app.emit("settings-updated", &settings);
    Ok(settings)
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedCreds {
    username: String,
    remember_password: bool,
    auto_login: bool,
}

#[tauri::command]
fn saved_creds(state: State<AppState>) -> SavedCreds {
    let s = state.settings.lock().unwrap().clone();
    let username = if !s.last_username.is_empty() {
        s.last_username.clone()
    } else {
        state
            .snapshot
            .lock()
            .unwrap()
            .username
            .clone()
            .unwrap_or_default()
    };
    SavedCreds {
        username,
        remember_password: s.remember_password,
        auto_login: s.auto_login,
    }
}

#[tauri::command]
fn saved_password(state: State<AppState>) -> Option<String> {
    let s = state.settings.lock().unwrap().clone();
    if !s.remember_password || s.last_username.is_empty() {
        return None;
    }
    keyring_get(&s.last_username)
}

fn remember_login(state: &AppState, username: &str, password: &str, remember: bool, auto: bool) {
    let mut s = state.settings.lock().unwrap();
    s.last_username = username.to_string();
    s.remember_password = remember;
    s.auto_login = remember && auto;
    if remember {
        let _ = keyring_set(username, password);
    } else {
        keyring_delete(username);
        s.auto_login = false;
    }
    save_settings_file(&state.data_dir, &s);
}

fn mark_logged_in(state: &AppState, username: &str) {
    mark_session_open(&state.data_dir);
    let mut snap = state.snapshot.lock().unwrap();
    snap.logged_in = true;
    snap.offline = false;
    snap.username = Some(username.to_string());
    snap.errors.auth = None;
    save_snapshot_file(&state.data_dir, &snap);
}

fn reveal_running_instance(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_decorations(false);
        let _ = w.set_shadow(false);
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_always_on_top(true);
        crate::hotkey::raise_noactivate(&w);
    }
    let _ = app.emit("island-reveal", ());
    let _ = open_desk_window(app);
}

fn spawn_refresh(app: AppHandle) {
    let _ = app.emit("auth-status", "syncing");
    tauri::async_runtime::spawn(async move {
        if let Some(state) = app.try_state::<AppState>() {
            match refresh_inner(&state, true).await {
                Ok(snap) => {
                    let status = if snap.offline {
                        "idle"
                    } else if snap.logged_in {
                        "ok"
                    } else {
                        "idle"
                    };
                    emit_snapshot(&app, &snap);
                    let _ = app.emit("auth-status", status);
                }
                Err(e) => {
                    let _ = app.emit("auth-status", format!("fail:{e}"));
                }
            }
        }
    });
}

#[tauri::command]
async fn login(
    app: AppHandle,
    state: State<'_, AppState>,
    username: String,
    password: String,
    remember_password: Option<bool>,
    auto_login: Option<bool>,
) -> Result<LoginOutcome> {
    let _ = (app, state, username, password, remember_password, auto_login);
    Err(SCHOOL_DATA_UNAVAILABLE.into())
}

#[tauri::command]
async fn refresh_captcha(_state: State<'_, AppState>) -> Result<String> {
    Err(SCHOOL_DATA_UNAVAILABLE.into())
}

#[tauri::command]
async fn send_sms(_state: State<'_, AppState>, _captcha: String) -> Result<()> {
    Err(SCHOOL_DATA_UNAVAILABLE.into())
}

#[tauri::command]
async fn cancel_sms(state: State<'_, AppState>) -> Result<()> {
    *state.pending_login.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
async fn submit_sms(
    _app: AppHandle,
    _state: State<'_, AppState>,
    _sms: String,
    _trust: bool,
) -> Result<LoginOutcome> {
    Err(SCHOOL_DATA_UNAVAILABLE.into())
}

#[tauri::command]
fn set_bills_open(state: State<AppState>, open: bool) {
    state.pause_bills.store(open, Ordering::SeqCst);
}

#[tauri::command]
async fn refresh(_app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot> {
    let mut snap = state.snapshot.lock().unwrap().clone();
    clear_school_snapshot(&mut snap);
    attach_local(&state.data_dir, &mut snap);
    *state.snapshot.lock().unwrap() = snap.clone();
    Ok(snap)
}

#[tauri::command]
async fn load_month(_state: State<'_, AppState>, _year: i32, _month: u32) -> Result<Snapshot> {
    Err(SCHOOL_DATA_UNAVAILABLE.into())
}

async fn refresh_inner(state: &AppState, _retry_courses: bool) -> Result<Snapshot> {
    let mut snap = state.snapshot.lock().unwrap().clone();
    clear_school_snapshot(&mut snap);
    attach_local(&state.data_dir, &mut snap);
    *state.snapshot.lock().unwrap() = snap.clone();
    Ok(snap)
}
#[tauri::command]
async fn sync_courses(_app: AppHandle, _state: State<'_, AppState>) -> Result<Snapshot> {
    Err(SCHOOL_DATA_UNAVAILABLE.into())
}
static DESK_PLACED: AtomicBool = AtomicBool::new(false);

fn center_desk_on_island(app: &AppHandle, desk: &WebviewWindow) {
    let mon = app
        .get_webview_window("main")
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| desk.current_monitor().ok().flatten());
    let Some(m) = mon else {
        let _ = desk.center();
        return;
    };
    let area = m.work_area();
    let scale = m.scale_factor();
    let sw = area.size.width as f64 / scale;
    let sh = area.size.height as f64 / scale;
    let mx = area.position.x as f64 / scale;
    let my = area.position.y as f64 / scale;
    let (ww, wh) = desk
        .outer_size()
        .ok()
        .map(|s| (s.width as f64 / scale, s.height as f64 / scale))
        .unwrap_or((1100.0, 720.0));
    let x = mx + ((sw - ww) / 2.0).max(0.0);
    let y = my + ((sh - wh) / 2.0).max(0.0);
    let _ = desk.set_position(LogicalPosition::new(x, y));
}

fn open_desk_on_ui(app: &AppHandle) -> Result<()> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "应用状态尚未初始化".to_string())?;
    // SetForegroundWindow 会泵消息。连点若在这里阻塞等同一把锁，主线程就死锁，窗口显示未响应。
    let _open_guard = match state.desk_open_lock.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
    };
    let Some(desk) = app.get_webview_window("desk") else {
        return Err("找不到工作区窗口".into());
    };
    if !DESK_PLACED.swap(true, Ordering::SeqCst) {
        center_desk_on_island(app, &desk);
    }
    let _ = desk.unminimize();
    desk.show().map_err(|e| e.to_string())?;
    // 窗口配置里先隐藏。wry 创建时登记的拖放接不住后来出现的 WebView 子窗口。
    crate::desk_drop::arm(app);
    // 在 UI 线程里只调用 SetForegroundWindow。set_focus 失败时会注入 Alt，
    // 岛还是前台时这个 Alt 会打开系统标题栏，连点会把消息循环卡死。
    crate::hotkey::focus_without_menu(&desk);
    Ok(())
}

fn open_desk_window(app: &AppHandle) -> Result<()> {
    let app2 = app.clone();
    app.run_on_main_thread(move || {
        let _ = open_desk_on_ui(&app2);
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_desk(app: AppHandle) -> Result<()> {
    open_desk_window(&app)
}

/// 任意门。`policy`: direct 原样打开；intranet 校园网直连、校外走 WebVPN；campus-only 不在校园网就拒绝。
#[tauri::command]
async fn open_door(app: AppHandle, url: String, policy: String) -> Result<()> {
    let url = url.trim().to_string();
    let allowed = matches!(
        url.as_str(),
        "https://www.cwxu.edu.cn/"
            | "https://www.cwxu.edu.cn"
            | "https://jwgl.cwxu.edu.cn/jwglxt/xtgl/login_slogin.htm"
            | "https://jwgl.cwxu.edu.cn/jwglxt/kbcx/xskbcx_cxXsgrkb.html?gnmkdm=N2151"
    );
    if policy != "direct" || !allowed {
        return Err("当前只开放已核实的无锡学院官网和教务系统页面".into());
    }
    let target = url;
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(target, None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn current_wifi() -> std::result::Result<String, String> {
    let now = tokio::task::spawn_blocking(crate::wlan::query)
        .await
        .map_err(|_| "没有无线网卡".to_string())??;
    if !now.has_adapter {
        return Err("没有无线网卡".into());
    }
    let name = now.ssid.trim();
    if !now.connected || name.is_empty() {
        return Err("现在没有连上无线".into());
    }
    Ok(name.to_string())
}

#[tauri::command]
fn open_feedback(_app: AppHandle) -> Result<()> {
    Err("反馈入口尚未配置".into())
}

#[tauri::command]
fn preview_remind(app: AppHandle) {
    let _ = app.emit("preview-remind", ());
}

#[tauri::command]
fn persist_island(state: State<AppState>, win: WebviewWindow) -> Result<()> {
    if let Ok(pos) = win.outer_position() {
        let scale = win.scale_factor().unwrap_or(1.0);
        let (x, y) = clamp_pos(&win, pos.x as f64 / scale, pos.y as f64 / scale);
        let _ = win.set_position(LogicalPosition::new(x, y));
        crate::hotkey::clip_round(&win);
        let mut s = state.settings.lock().unwrap();
        s.island_x = Some(x);
        s.island_y = Some(y);
        save_settings_file(&state.data_dir, &s);
    }
    Ok(())
}

#[tauri::command]
fn reset_island(app: AppHandle, state: State<AppState>, win: WebviewWindow) -> Result<()> {
    let _ = win.show();
    win.set_size(LogicalSize::new(420.0, 40.0))
        .map_err(|e| e.to_string())?;
    {
        let mut s = state.settings.lock().unwrap();
        s.island_x = None;
        s.island_y = None;
        save_settings_file(&state.data_dir, &s);
        place_window(&win, &s);
    }
    let _ = win.set_always_on_top(true);
    crate::hotkey::raise_noactivate(&win);
    let _ = app.emit("island-reset", ());
    Ok(())
}

#[tauri::command]
fn raise_topmost(win: WebviewWindow) -> Result<()> {
    let _ = win.set_decorations(false);
    let _ = win.set_shadow(false);
    let _ = win.show();
    let _ = win.set_always_on_top(true);
    crate::hotkey::raise_noactivate(&win);
    Ok(())
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HoverWatch {
    enabled: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[tauri::command]
fn set_hover_watch(watch: HoverWatch) {
    crate::hotkey::set_hover_watch(watch.enabled, watch.x, watch.y, watch.width, watch.height);
}

#[tauri::command]
async fn save_agenda(app: AppHandle, state: State<'_, AppState>, item: AgendaItem) -> Result<Snapshot> {
    let _gate = state.refresh_gate.lock().await;
    let items = agenda::upsert(&state.data_dir, item)?;
    let mut snap = state.snapshot.lock().unwrap();
    snap.agenda = items;
    let out = snap.clone();
    drop(snap);
    emit_snapshot(&app, &out);
    Ok(out)
}

#[tauri::command]
async fn delete_agenda(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<Snapshot> {
    let _gate = state.refresh_gate.lock().await;
    let items = agenda::delete(&state.data_dir, &id)?;
    let mut snap = state.snapshot.lock().unwrap();
    snap.agenda = items;
    let out = snap.clone();
    drop(snap);
    emit_snapshot(&app, &out);
    Ok(out)
}

#[tauri::command]
async fn logout(app: AppHandle, state: State<'_, AppState>, keep_agenda: bool) -> Result<()> {
    state.epoch.fetch_add(1, Ordering::SeqCst);
    let remember = state.settings.lock().unwrap().remember_password;
    let user = {
        let g = state.client.lock().await;
        g.as_ref()
            .and_then(|c| c.username())
            .or_else(|| state.snapshot.lock().unwrap().username.clone())
    };
    *state.client.lock().await = None;
    *state.snapshot.lock().unwrap() = Snapshot::default();
    *state.pending_login.lock().unwrap() = None;
    wipe_local_data(&state.data_dir, keep_agenda);
    if keep_agenda {
        let mut snap = state.snapshot.lock().unwrap();
        attach_local(&state.data_dir, &mut snap);
    }
    if !remember {
        if let Some(u) = user {
            keyring_delete(&u);
        }
    }
    let snap = state.snapshot.lock().unwrap().clone();
    emit_snapshot(&app, &snap);
    Ok(())
}

fn emit_logged_out_keep_cache(app: &AppHandle, state: &AppState, reason: &str) {
    let mut snap = state.snapshot.lock().unwrap().clone();
    snap.logged_in = false;
    snap.offline = false;
    if snap.errors.auth.is_none() {
        snap.errors.auth = Some(reason.into());
    }
    attach_local(&state.data_dir, &mut snap);
    save_snapshot_file(&state.data_dir, &snap);
    *state.snapshot.lock().unwrap() = snap.clone();
    emit_snapshot(&app, &snap);
    let _ = app.emit("auth-status", "idle");
}

fn emit_offline(app: &AppHandle, state: &AppState) {
    let snap = apply_offline(state);
    emit_snapshot(&app, &snap);
    let _ = app.emit("auth-status", "idle");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        reveal_running_instance(app);
    }));
    builder = builder.plugin(tauri_plugin_opener::init());
    builder = builder.plugin(tauri_plugin_dialog::init());
    builder = builder.plugin(tauri_plugin_process::init());
    builder
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let _ = fs::create_dir_all(&data_dir);
            crate::clock::init(data_dir.clone());
            let mut settings = load_settings(&data_dir);
            settings.room_no.clear();
            settings.week1_monday.clear();
            settings.class_remind = false;
            settings.last_username.clear();
            settings.remember_password = false;
            settings.auto_login = false;
            settings.campus_wifi_mode = "off".into();
            settings.campus_wifi_ssid.clear();
            settings.campus_hotspot_ssid.clear();
            settings.skip_login = true;
            save_settings_file(&data_dir, &settings);
            let _ = apply_autostart(settings.autostart);
            crate::hotkey::init(app.handle().clone());
            crate::hotkey::update(&settings);
            let mut snapshot: Snapshot = fs::read(data_dir.join("snapshot.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            clear_school_snapshot(&mut snapshot);
            attach_local(&data_dir, &mut snapshot);
            app.manage(share::ShareHub::default());
            app.manage(AppState {
                client: AsyncMutex::new(None),
                epoch: AtomicU64::new(0),
                settings: Mutex::new(settings.clone()),
                snapshot: Mutex::new(snapshot),
                pending_login: Mutex::new(None),
                pause_bills: AtomicBool::new(false),
                desk_open_lock: Mutex::new(()),
                refresh_gate: AsyncMutex::new(()),
                course_attempt_day: Mutex::new(None),
                data_dir: data_dir.clone(),
                tray_balances: Mutex::new(None),
            });

            if let Some(win) = app.get_webview_window("main") {
                place_window(&win, &settings);
                crate::hotkey::clip_round(&win);
                crate::hotkey::block_island_caption(&win);
                let handle = app.handle().clone();
                let move_gen = Arc::new(AtomicU64::new(0));
                let move_echo = Arc::new(AtomicBool::new(false));
                let _ = win.on_window_event(move |ev| {
                    let Some(w) = handle.get_webview_window("main") else {
                        return;
                    };
                    if matches!(ev, tauri::WindowEvent::Resized(_)) {
                        crate::hotkey::clip_round(&w);
                    }
                    if let tauri::WindowEvent::Moved(p) = ev {
                        // set_position 会再发一条 Moved。若再纠正，DPI 取整会把消息循环打满，窗口未响应。
                        if move_echo.swap(false, Ordering::SeqCst) {
                            crate::hotkey::clip_round(&w);
                            return;
                        }
                        let scale = w.scale_factor().unwrap_or(1.0);
                        let raw_x = p.x as f64 / scale;
                        let raw_y = p.y as f64 / scale;
                        let (x, y) = clamp_pos(&w, raw_x, raw_y);
                        if (x - raw_x).abs() > 2.0 || (y - raw_y).abs() > 2.0 {
                            move_echo.store(true, Ordering::SeqCst);
                            let _ = w.set_position(LogicalPosition::new(x, y));
                        }
                        crate::hotkey::clip_round(&w);
                        if let Some(state) = handle.try_state::<AppState>() {
                            let mut s = state.settings.lock().unwrap();
                            s.island_x = Some(x);
                            s.island_y = Some(y);
                        }
                        let n = move_gen.fetch_add(1, Ordering::Relaxed) + 1;
                        let handle2 = handle.clone();
                        let gen = move_gen.clone();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(350)).await;
                            if gen.load(Ordering::Relaxed) != n {
                                return;
                            }
                            if let Some(state) = handle2.try_state::<AppState>() {
                                let s = state.settings.lock().unwrap().clone();
                                save_settings_file(&state.data_dir, &s);
                            }
                        });
                    }
                });
            }

            if let Some(desk) = app.get_webview_window("desk") {
                let win = desk.clone();
                desk.on_window_event(move |ev| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            let snap_now = app.state::<AppState>().snapshot.lock().unwrap().clone();
            let (card_t, net_t, elec_t) = tray_labels(&snap_now);
            let card = MenuItem::with_id(app, "bal-card", card_t, false, None::<&str>)?;
            let net = MenuItem::with_id(app, "bal-net", net_t, false, None::<&str>)?;
            let elec = MenuItem::with_id(app, "bal-elec", elec_t, false, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "打开窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let reset = MenuItem::with_id(app, "reset", "复位岛", true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "刷新（数据待接入）", false, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::new(app)?;
            menu.append(&card)?;
            menu.append(&net)?;
            menu.append(&elec)?;
            menu.append(&sep)?;
            menu.append(&open)?;
            menu.append(&refresh)?;
            menu.append(&reset)?;
            menu.append(&quit)?;
            *app.state::<AppState>().tray_balances.lock().unwrap() = Some(TrayBalances {
                card: card.clone(),
                net: net.clone(),
                elec: elec.clone(),
            });
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("无锡学院桌面看板")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|icon, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = open_desk_window(icon.app_handle());
                    }
                });
            // Use the native 32px mark (not a downscaled 128/ICO frame) so the
            // notification-area icon stays sharp at 100–150% DPI.
            tray = tray.icon(Image::new_owned(
                include_bytes!("../icons/tray-32.rgba").to_vec(),
                32,
                32,
            ));
            let _tray = tray
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "open" => {
                        let _ = open_desk_window(app);
                    }
                    "reset" => {
                        if let Some(w) = app.get_webview_window("main") {
                            if let Some(state) = app.try_state::<AppState>() {
                                let _ = w.show();
                                let _ = w.set_size(LogicalSize::new(420.0, 40.0));
                                let mut s = state.settings.lock().unwrap();
                                s.island_x = None;
                                s.island_y = None;
                                save_settings_file(&state.data_dir, &s);
                                place_window(&w, &s);
                                let _ = w.set_always_on_top(true);
                                crate::hotkey::raise_noactivate(&w);
                            }
                        }
                        let _ = app.emit("island-reset", ());
                    }
                    "refresh" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Some(state) = app.try_state::<AppState>() {
                                if let Ok(snap) = refresh_inner(&state, true).await {
                                    emit_snapshot(&app, &snap);
                                }
                            }
                        });
                    }
                    _ => {}
                })
                .build(app)?;
            // Campus Wi-Fi automation remains disabled until an official Wuxi integration exists.
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            clock_now,
            get_settings,
            save_settings,
            save_local_profile,
            clear_local_jwxt_credentials,
            saved_creds,
            saved_password,
            persist_island,
            login,
            refresh_captcha,
            send_sms,
            cancel_sms,
            submit_sms,
            refresh,
            load_month,
            sync_courses,
            open_desk,
            open_door,
            preview_remind,
            current_wifi,
            open_feedback,
            reset_island,
            raise_topmost,
            set_hover_watch,
            set_bills_open,
            save_agenda,
            delete_agenda,
            logout,
            share::share_describe,
            share::share_start,
            share::share_stop,
            share::share_offer,
            share::share_revise,
            share::share_on_campus
        ])
        .run(tauri::generate_context!())
        .expect("无锡学院桌面看板启动失败");
}

#[cfg(test)]
mod tests {
    use super::{elec_night, settle_elec, ElecInfo};

    #[test]
    fn elec_quiet_hours_are_23_to_6() {
        assert!(elec_night(23));
        assert!(elec_night(0));
        assert!(elec_night(5));
        assert!(!elec_night(6));
        assert!(!elec_night(11));
        assert!(!elec_night(22));
    }

    fn reading(room: Option<&str>, kwh: f64) -> ElecInfo {
        ElecInfo {
            unbound: false,
            remain_kwh: Some(kwh),
            room_no: room.map(str::to_string),
            room_label: None,
            last_read_at: None,
            stale: false,
        }
    }

    #[test]
    fn night_failure_keeps_the_reading_and_drops_the_error() {
        let mut elec = Some(reading(Some("351-0409"), 182.0));
        let mut err = Some("旧的同步失败".into());
        settle_elec(&mut elec, &mut err, "3510409", true, "同步超时，请再刷新一次。");
        let kept = elec.expect("cache");
        assert_eq!(kept.remain_kwh, Some(182.0));
        assert!(kept.stale);
        assert!(err.is_none());
    }

    #[test]
    fn night_failure_keeps_a_reading_that_never_stored_its_room() {
        let mut elec = Some(reading(None, 12.5));
        let mut err = None;
        settle_elec(&mut elec, &mut err, "3510409", true, "电费系统维护");
        let kept = elec.expect("legacy cache");
        assert_eq!(kept.remain_kwh, Some(12.5));
        assert!(kept.stale);
        assert_eq!(kept.room_no.as_deref(), Some("3510409"));
        assert!(err.is_none());
    }

    #[test]
    fn day_failure_keeps_the_number_but_marks_sync_failed() {
        let mut elec = Some(reading(Some("3510409"), 40.0));
        let mut err = None;
        settle_elec(&mut elec, &mut err, "3510409", false, "查电费失败");
        let kept = elec.expect("day cache");
        assert_eq!(kept.remain_kwh, Some(40.0));
        assert!(!kept.stale);
        assert_eq!(err.as_deref(), Some("查电费失败"));
    }

    #[test]
    fn a_different_room_drops_the_old_reading() {
        let mut elec = Some(reading(Some("3510409"), 40.0));
        let mut err = None;
        settle_elec(&mut elec, &mut err, "3520101", true, "查电费失败");
        assert!(elec.is_none());
        assert!(err.is_some());
    }
}
