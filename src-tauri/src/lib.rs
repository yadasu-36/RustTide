mod native;
mod settings;
mod translation;

use serde::Serialize;
use serde_json::{json, Value};
use settings::Settings;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Capture {
    id: u64,
    text: String,
    replaceable: bool,
}
struct AppState {
    settings: Mutex<Settings>,
    config_path: PathBuf,
    capture: Mutex<Option<Capture>>,
    cancellation: Mutex<Option<CancellationToken>>,
    busy: AtomicBool,
    sequence: AtomicU64,
    startup_error: Mutex<Option<String>>,
}
struct Busy<'a>(&'a AppState);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        *self.0.cancellation.lock().unwrap() = None;
        self.0.busy.store(false, Ordering::SeqCst);
    }
}
impl AppState {
    fn begin(&self) -> Result<(Busy<'_>, CancellationToken), String> {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "当前操作尚未结束，请等待或取消。")?;
        let cancel = CancellationToken::new();
        *self.cancellation.lock().unwrap() = Some(cancel.clone());
        Ok((Busy(self), cancel))
    }
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}
fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn event(app: &AppHandle, value: Value) {
    let _ = app.emit("operation", value);
}
fn shortcuts(settings: &Settings) -> Result<(Shortcut, Shortcut), String> {
    let read: Shortcut = settings
        .read_shortcut
        .parse()
        .map_err(|_| "取词快捷键格式无效，例如 Alt+D。")?;
    let replace: Shortcut = settings
        .replace_shortcut
        .parse()
        .map_err(|_| "替换快捷键格式无效，例如 Alt+R。")?;
    if read == replace {
        return Err("两个快捷键不能相同。".into());
    }
    // Avoid ordinary unmodified keys becoming global shortcuts.
    for raw in [&settings.read_shortcut, &settings.replace_shortcut] {
        if !raw.contains('+') {
            return Err("全局快捷键需要包含修饰键，例如 Alt+D。".into());
        }
    }
    Ok((read, replace))
}
fn register_shortcuts(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let (read, replace) = shortcuts(settings)?;
    app.global_shortcut()
        .register(read)
        .map_err(|_| "取词快捷键注册失败，请换一个组合。")?;
    if app.global_shortcut().register(replace).is_err() {
        let _ = app.global_shortcut().unregister(read);
        return Err("替换快捷键注册失败，请换一个组合。".into());
    }
    Ok(())
}
async fn api(
    app: &AppHandle,
    settings: Settings,
    text: &str,
    cancel: &CancellationToken,
) -> Result<String, String> {
    let account = settings.key_account()?;
    let system_proxy = settings.use_system_proxy;
    let (key, proxy) = native::on_main(app, move || {
        Ok((
            native::key_get(&account)?,
            if system_proxy {
                native::system_proxy()?
            } else {
                None
            },
        ))
    })
    .await?;
    translation::translate(&settings, &key, text, proxy.as_deref(), cancel).await
}
async fn capture_flow(app: AppHandle, direct: bool, delay: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (_busy, cancel) = state.begin()?;
    *state.capture.lock().unwrap() = None;
    event(
        &app,
        json!({"kind":"started", "delayed":delay, "direct":direct}),
    );
    if delay {
        tokio::select! {
            _ = cancel.cancelled() => return Err("已取消。".into()),
            _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
        }
    } else {
        // Give the user time to release the shortcut modifiers before simulated copy.
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }
    if cancel.is_cancelled() {
        return Err("已取消。".into());
    }
    let settings = state.settings();
    let whole = settings.capture_whole_input;
    let clipboard = settings.clipboard_fallback;
    let capture = native::on_main(&app, move || native::capture(whole, clipboard)).await?;
    if cancel.is_cancelled() {
        return Err("已取消。".into());
    }
    let text = capture["text"]
        .as_str()
        .ok_or("没有取得文字。")?
        .to_string();
    translation::validate_text(&text)?;
    let id = state.sequence.fetch_add(1, Ordering::SeqCst) + 1;
    let replaceable = capture["replaceable"].as_bool().unwrap_or(false);
    *state.capture.lock().unwrap() = Some(Capture {
        id,
        text: text.clone(),
        replaceable,
    });
    event(
        &app,
        json!({"kind":"captured", "source":text, "captureId":id,
        "replaceable":replaceable, "app":capture["app"], "whole":capture["whole"], "method":capture["method"]}),
    );
    if !direct {
        show(&app);
    }
    let result = api(&app, settings, &text, &cancel).await?;
    if cancel.is_cancelled() {
        return Err("已取消。".into());
    }
    event(
        &app,
        json!({"kind":"translated", "result":result, "captureId":id, "direct":direct}),
    );
    if direct {
        if !replaceable {
            return Err("此处不能直接替换，译文已保留，请复制使用。".into());
        }
        let replacement = result.clone();
        let replaced = native::on_main(&app, move || native::replace(&replacement)).await?;
        *state.capture.lock().unwrap() = None;
        event(
            &app,
            json!({"kind":"replaced", "verified":replaced["verified"]}),
        );
    }
    Ok(())
}
fn start_capture(app: AppHandle, direct: bool, delay: bool) {
    if app.state::<AppState>().busy.load(Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(error) = capture_flow(app.clone(), direct, delay).await {
            if error == "当前操作尚未结束，请等待或取消。" {
                return;
            }
            event(&app, json!({"kind":"error", "message":error}));
            if error != "已取消。" {
                show(&app);
            }
        }
    });
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    settings: Settings,
    has_key: bool,
    accessibility: bool,
    startup_error: Option<String>,
}
#[tauri::command]
async fn get_settings(app: AppHandle, state: State<'_, AppState>) -> Result<SettingsView, String> {
    let settings = state.settings();
    let account = settings.key_account()?;
    let (has_key, accessibility) = native::on_main(&app, move || {
        Ok((
            !native::key_get(&account)?.is_empty(),
            native::permission(false)?,
        ))
    })
    .await?;
    Ok(SettingsView {
        settings,
        has_key,
        accessibility,
        startup_error: state.startup_error.lock().unwrap().clone(),
    })
}
#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mut settings: Settings,
    api_key: Option<String>,
    delete_key: bool,
) -> Result<(), String> {
    let (_busy, _) = state.begin()?;
    settings.validate()?;
    shortcuts(&settings)?;
    let old = state.settings();
    let changed = old.read_shortcut != settings.read_shortcut
        || old.replace_shortcut != settings.replace_shortcut
        || state.startup_error.lock().unwrap().is_some();
    if changed {
        app.global_shortcut()
            .unregister_all()
            .map_err(|_| "无法更新快捷键。")?;
        if let Err(error) = register_shortcuts(&app, &settings) {
            if let Err(rollback) = register_shortcuts(&app, &old) {
                *state.startup_error.lock().unwrap() = Some(rollback);
            }
            return Err(error);
        }
    }
    let account = settings.key_account()?;
    let persist = async {
        native::on_main(&app, move || {
            if delete_key {
                native::key_delete(&account)?;
            } else if let Some(key) = api_key.filter(|key| !key.trim().is_empty()) {
                if key.len() > 4096 {
                    return Err("API 密钥过长。".into());
                }
                native::key_set(&account, key.trim())?;
            }
            Ok(())
        })
        .await?;
        settings.save(&state.config_path)
    }
    .await;
    if let Err(error) = persist {
        if changed {
            let _ = app.global_shortcut().unregister_all();
            if let Err(rollback) = register_shortcuts(&app, &old) {
                *state.startup_error.lock().unwrap() = Some(rollback);
            }
        }
        return Err(error);
    }
    *state.settings.lock().unwrap() = settings;
    *state.startup_error.lock().unwrap() = None;
    Ok(())
}
#[tauri::command]
async fn accessibility_status(app: AppHandle) -> Result<bool, String> {
    native::on_main(&app, || native::permission(false)).await
}
#[tauri::command]
async fn request_permission(app: AppHandle) -> Result<bool, String> {
    let trusted = native::on_main(&app, || native::permission(true)).await?;
    if !trusted {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|_| "无法打开系统设置，请手动进入辅助功能。")?;
    }
    Ok(trusted)
}
#[tauri::command]
fn capture_later(app: AppHandle) {
    start_capture(app, false, true);
}
#[tauri::command]
fn cancel_operation(state: State<'_, AppState>) {
    if let Some(cancel) = state.cancellation.lock().unwrap().as_ref() {
        cancel.cancel();
    }
}
#[tauri::command]
async fn translate_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    language: String,
    style: String,
    capture_id: Option<u64>,
) -> Result<Value, String> {
    let (_busy, cancel) = state.begin()?;
    translation::validate_text(&text)?;
    if let Some(id) = capture_id {
        if !state
            .capture
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|capture| capture.id == id && capture.text == text)
        {
            return Err("取词记录已失效，请重新取词。".into());
        }
    } else {
        *state.capture.lock().unwrap() = None;
    }
    let mut settings = state.settings();
    settings.target_language = language;
    settings.style = style;
    settings.validate()?;
    let result = api(&app, settings, &text, &cancel).await?;
    Ok(json!({"result":result, "captureId":capture_id}))
}
#[tauri::command]
async fn test_api(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let (_busy, cancel) = state.begin()?;
    api(&app, state.settings(), "你好，世界！", &cancel).await
}
#[tauri::command]
async fn replace_result(
    app: AppHandle,
    state: State<'_, AppState>,
    capture_id: u64,
    text: String,
) -> Result<Value, String> {
    let (_busy, _) = state.begin()?;
    translation::validate_text(&text)?;
    if !state
        .capture
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|capture| capture.id == capture_id && capture.replaceable)
    {
        return Err("取词目标已失效或不可写，请重新取词或复制译文。".into());
    }
    let result = native::on_main(&app, move || native::replace(&text)).await?;
    *state.capture.lock().unwrap() = None;
    Ok(result)
}
#[tauri::command]
async fn copy_text(app: AppHandle, state: State<'_, AppState>, text: String) -> Result<(), String> {
    let (_busy, _) = state.begin()?;
    native::on_main(&app, move || native::copy(&text))
        .await
        .map(|_| ())
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let settings = app.state::<AppState>().settings();
                        if let Ok((read, replace)) = shortcuts(&settings) {
                            if shortcut == &read {
                                start_capture(app.clone(), false, false);
                            } else if shortcut == &replace {
                                start_capture(app.clone(), true, false);
                            }
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            accessibility_status,
            request_permission,
            capture_later,
            cancel_operation,
            translate_text,
            test_api,
            replace_result,
            copy_text
        ])
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("settings.json");
            let (settings, load_error) = match Settings::load(&config_path) {
                Ok(settings) => (settings, None),
                Err(error) => (Settings::default(), Some(error)),
            };
            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                config_path,
                capture: Mutex::new(None),
                cancellation: Mutex::new(None),
                busy: AtomicBool::new(false),
                sequence: AtomicU64::new(0),
                startup_error: Mutex::new(load_error),
            });
            if let Err(error) = register_shortcuts(app.handle(), &settings) {
                *app.state::<AppState>().startup_error.lock().unwrap() = Some(error);
            }
            let open = MenuItem::with_id(app, "show", "打开 RustTide", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 RustTide", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("RustTide · Option+D 翻译 · Option+R 替换")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if let Some(window) = app.get_webview_window("main") {
                let handle = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = handle.hide();
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Unable to start RustTide");
}
