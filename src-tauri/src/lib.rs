mod native;
mod routing;
mod settings;
mod translation;

use routing::Trigger;
use serde::Serialize;
use serde_json::{json, Value};
use settings::{Output, Settings};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex, OnceLock,
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
    result: Option<String>,
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
        let cancel = self.claim()?;
        Ok((Busy(self), cancel))
    }
    fn claim(&self) -> Result<CancellationToken, String> {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "当前操作尚未结束，请等待或取消。")?;
        let cancel = CancellationToken::new();
        *self.cancellation.lock().unwrap() = Some(cancel.clone());
        Ok(cancel)
    }
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
    fn cancel(&self) {
        if let Some(cancel) = self.cancellation.lock().unwrap().as_ref() {
            cancel.cancel();
        }
    }
}
fn show(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.cancel();
        *state.capture.lock().unwrap() = None;
    }
    let _ = app.run_on_main_thread(native::popup_hide);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn shortcuts(settings: &Settings) -> Result<(Shortcut, Shortcut), String> {
    let read: Shortcut = settings
        .read_shortcut
        .parse()
        .map_err(|_| "读取快捷键格式无效，例如 Alt+D。")?;
    let write: Shortcut = settings
        .replace_shortcut
        .parse()
        .map_err(|_| "写入快捷键格式无效，例如 Alt+R。")?;
    if read == write {
        return Err("两个快捷键不能相同。".into());
    }
    for raw in [&settings.read_shortcut, &settings.replace_shortcut] {
        if !raw.contains('+') {
            return Err("全局快捷键需要包含修饰键，例如 Alt+D。".into());
        }
    }
    Ok((read, write))
}
fn register_shortcuts(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let (read, write) = shortcuts(settings)?;
    app.global_shortcut()
        .register(read)
        .map_err(|_| "读取快捷键注册失败，请换一个组合。")?;
    if app.global_shortcut().register(write).is_err() {
        let _ = app.global_shortcut().unregister(read);
        return Err("写入快捷键注册失败，请换一个组合。".into());
    }
    Ok(())
}
async fn popup_update(app: &AppHandle, value: Value) -> Result<(), String> {
    native::on_main(app, move || native::popup_update(&value)).await
}
async fn api(
    app: &AppHandle,
    settings: Settings,
    text: &str,
    language: &str,
    cancel: &CancellationToken,
    popup: Option<u64>,
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
    let mut last_update = std::time::Instant::now() - std::time::Duration::from_millis(40);
    translation::translate(
        &settings,
        &key,
        text,
        language,
        proxy.as_deref(),
        cancel,
        |result| {
            if let Some(id) = popup {
                if last_update.elapsed() < std::time::Duration::from_millis(40) {
                    return;
                }
                last_update = std::time::Instant::now();
                let payload = json!({"id":id, "kind":"stream", "result":result});
                let _ = app.run_on_main_thread(move || {
                    let _ = native::popup_update(&payload);
                });
            }
        },
    )
    .await
}
async fn capture_flow(
    app: &AppHandle,
    trigger: Trigger,
    id: u64,
    cancel: CancellationToken,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    *state.capture.lock().unwrap() = None;
    native::on_main(app, || {
        native::popup_hide();
        native::clear_capture();
        Ok(())
    })
    .await?;
    let _ = app.emit("operation", json!({"busy":true}));
    // Release shortcut modifiers before a possible native copy/paste.
    tokio::select! {
        _ = cancel.cancelled() => return Err("已取消。".into()),
        _ = tokio::time::sleep(std::time::Duration::from_millis(120)) => {}
    }
    let settings = state.settings();
    let whole = trigger.whole_input(&settings);
    let clipboard = settings.clipboard_fallback;
    let excluded = settings.excluded_apps.clone();
    let capture = native::on_main(app, move || {
        if excluded.contains(&native::frontmost_bundle()?) {
            return Err("此应用已在排除列表中，未读取文字。".into());
        }
        native::capture(whole, clipboard)
    })
    .await?;
    if cancel.is_cancelled() {
        return Err("已取消。".into());
    }
    let text = capture["text"]
        .as_str()
        .ok_or("没有取得文字。")?
        .to_string();
    translation::validate_text(&text)?;
    let replaceable = capture["replaceable"].as_bool().unwrap_or(false);
    let to_detect = text.clone();
    let detected = native::on_main(app, move || native::detect_language(&to_detect)).await?;
    let language = trigger.target(&settings, &detected);
    *state.capture.lock().unwrap() = Some(Capture {
        id,
        result: None,
        replaceable,
    });
    let direct = trigger.output(&settings) == Output::Replace;
    let payload = json!({"id":id,"source":text,"language":language,"direct":direct});
    let token = cancel.clone();
    native::on_main(app, move || {
        if token.is_cancelled() {
            return Err("已取消。".into());
        }
        native::popup_show(&payload)
    })
    .await?;
    let result = api(app, settings, &text, &language, &cancel, Some(id)).await?;
    if cancel.is_cancelled() {
        return Err("已取消。".into());
    }
    if let Some(capture) = state
        .capture
        .lock()
        .unwrap()
        .as_mut()
        .filter(|c| c.id == id)
    {
        capture.result = Some(result.clone());
    }
    if direct {
        if !replaceable {
            return Err("这里不是可写输入框。译文已保留，可复制使用。".into());
        }
        let replacement = result.clone();
        let token = cancel.clone();
        let replaced = native::on_main(app, move || {
            if token.is_cancelled() {
                return Err("已取消。".into());
            }
            native::replace(&replacement)
        })
        .await?;
        *state.capture.lock().unwrap() = None;
        if replaced["verified"].as_bool() == Some(true) {
            native::on_main(app, || {
                native::popup_hide();
                Ok(())
            })
            .await?;
        } else {
            popup_update(
                app,
                json!({"id":id,"kind":"error","result":result,"complete":false,"replaceable":false,
                "message":"已发送粘贴，请在原应用确认结果；可按 ⌘Z 撤销。"}),
            )
            .await?;
        }
    } else {
        popup_update(
            app,
            json!({"id":id,"kind":"done","result":result,"complete":true,"replaceable":replaceable,
            "message":if replaceable {"翻译完成 · 可替换原选区"} else {"翻译完成 · 可复制译文"}}),
        )
        .await?;
    }
    Ok(())
}
fn start_capture(app: AppHandle, trigger: Trigger) {
    // Claim before spawning; a repeated shortcut must not change the active ID.
    let state = app.state::<AppState>();
    let Ok(cancel) = state.claim() else { return };
    let id = state.sequence.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let busy = Busy(&state);
        if let Err(error) = capture_flow(&app, trigger, id, cancel).await {
            if error == "已取消。" {
                *app.state::<AppState>().capture.lock().unwrap() = None;
                let _ = native::on_main(&app, || {
                    native::popup_hide();
                    Ok(())
                })
                .await;
            } else {
                if let Some(capture) = app.state::<AppState>().capture.lock().unwrap().as_mut() {
                    capture.replaceable = false;
                }
                let saved = app.state::<AppState>().capture.lock().unwrap().clone();
                if saved.is_none() {
                    let payload = json!({"id":id,"source":"","language":"","direct":false});
                    let _ = native::on_main(&app, move || native::popup_show(&payload)).await;
                }
                let result = saved.and_then(|c| c.result);
                let _ = popup_update(
                    &app,
                    json!({"id":id,"kind":"error","result":result,"complete":result.is_some(),
                    "replaceable":false,"message":error}),
                )
                .await;
            }
        }
        drop(busy);
        let _ = app.emit("operation", json!({"busy":false}));
    });
}
static APP: OnceLock<AppHandle> = OnceLock::new();
extern "C" fn popup_action(action: i32, id: u64) {
    let Some(app) = APP.get().cloned() else {
        return;
    };
    let state = app.state::<AppState>();
    if id != state.sequence.load(Ordering::SeqCst) {
        return;
    }
    if action == 1 || action == 4 {
        state.cancel();
        *state.capture.lock().unwrap() = None;
        native::popup_hide(); // AppKit delivers the action on its main thread.
        if action == 4 {
            show(&app);
        }
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(error) = popup_result_action(&app, action, id).await {
            if let Some(capture) = app
                .state::<AppState>()
                .capture
                .lock()
                .unwrap()
                .as_mut()
                .filter(|c| c.id == id)
            {
                capture.replaceable = false;
            }
            let complete = app
                .state::<AppState>()
                .capture
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|c| c.id == id && c.result.is_some());
            let _ = popup_update(
                &app,
                json!({"id":id,"kind":"error","complete":complete,"replaceable":false,"message":error}),
            )
            .await;
        }
    });
}
async fn popup_result_action(app: &AppHandle, action: i32, id: u64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (_busy, cancel) = state.begin()?;
    let capture = state
        .capture
        .lock()
        .unwrap()
        .clone()
        .filter(|c| c.id == id)
        .ok_or("原取词记录已失效，请重新触发快捷键。")?;
    let result = capture.result.ok_or("译文尚未完整返回，请稍候。")?;
    match action {
        2 => {
            native::on_main(app, move || native::copy(&result)).await?;
            popup_update(app, json!({"id":id,"kind":"done","complete":true,"replaceable":capture.replaceable,"message":"译文已复制"})).await
        }
        3 if capture.replaceable => {
            popup_update(app, json!({"id":id,"kind":"busy","complete":false,"replaceable":false,"message":"正在替换…"})).await?;
            let replaced = native::on_main(app, move || {
                if cancel.is_cancelled() {
                    return Err("已取消。".into());
                }
                native::replace(&result)
            })
            .await?;
            *state.capture.lock().unwrap() = None;
            if replaced["verified"].as_bool() == Some(true) {
                native::on_main(app, || {
                    native::popup_hide();
                    Ok(())
                })
                .await
            } else {
                popup_update(
                    app,
                    json!({"id":id,"kind":"error","complete":false,"replaceable":false,
                    "message":"已发送粘贴，请在原应用确认；可按 ⌘Z 撤销。"}),
                )
                .await
            }
        }
        _ => Err("此处不能替换，请复制译文。".into()),
    }
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
fn cancel_operation(state: State<'_, AppState>) {
    state.cancel();
}
#[tauri::command]
async fn test_api(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let (_busy, cancel) = state.begin()?;
    let settings = state.settings();
    let language = settings.foreign_language.clone();
    api(&app, settings, "你好，世界！", &language, &cancel, None).await
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let settings = app.state::<AppState>().settings();
                        if let Ok((read, write)) = shortcuts(&settings) {
                            if shortcut == &read {
                                start_capture(app.clone(), Trigger::Read);
                            } else if shortcut == &write {
                                start_capture(app.clone(), Trigger::Write);
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
            cancel_operation,
            test_api
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let config_path = app.path().app_config_dir()?.join("settings.json");
            let first_run = !config_path.exists();
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
            let _ = APP.set(app.handle().clone());
            native::popup_init(popup_action);
            if let Err(error) = register_shortcuts(app.handle(), &settings) {
                *app.state::<AppState>().startup_error.lock().unwrap() = Some(error);
            }
            let open = MenuItem::with_id(app, "show", "设置…", true, None::<&str>)?;
            let cancel = MenuItem::with_id(app, "cancel", "取消当前翻译", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 RustTide", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &cancel, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("RustTide · 在当前输入框中翻译")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show(app),
                    "cancel" => {
                        app.state::<AppState>().cancel();
                    }
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
            if first_run
                || app
                    .state::<AppState>()
                    .startup_error
                    .lock()
                    .unwrap()
                    .is_some()
            {
                show(app.handle());
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Unable to start RustTide");
}

#[cfg(test)]
mod operation_tests {
    use super::*;
    #[test]
    fn refused_second_operation_preserves_first_token_and_busy_gate() {
        let state = AppState {
            settings: Mutex::new(Settings::default()),
            config_path: PathBuf::new(),
            capture: Mutex::new(None),
            cancellation: Mutex::new(None),
            busy: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            startup_error: Mutex::new(None),
        };
        let (guard, token) = state.begin().unwrap();
        assert!(state.begin().is_err());
        assert!(state.busy.load(Ordering::SeqCst));
        state.cancel();
        assert!(token.is_cancelled());
        drop(guard);
        assert!(!state.busy.load(Ordering::SeqCst));
        assert!(state.begin().is_ok());
    }
}
