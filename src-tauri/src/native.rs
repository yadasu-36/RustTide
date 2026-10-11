use serde_json::Value;
use std::ffi::{c_char, CStr, CString};
use tauri::AppHandle;

#[cfg(target_os = "macos")]
extern "C" {
    fn rt_free(value: *mut c_char);
    fn rt_permission(prompt: i32) -> i32;
    fn rt_capture(whole: i32, clipboard: i32) -> *mut c_char;
    fn rt_replace(text: *const c_char) -> *mut c_char;
    fn rt_copy(text: *const c_char) -> *mut c_char;
    fn rt_key_get(account: *const c_char) -> *mut c_char;
    fn rt_key_set(account: *const c_char, key: *const c_char) -> *mut c_char;
    fn rt_key_delete(account: *const c_char) -> *mut c_char;
    fn rt_system_proxy() -> *mut c_char;
    fn rt_frontmost_bundle() -> *mut c_char;
    fn rt_detect_language(text: *const c_char) -> *mut c_char;
    fn rt_popup_init(handler: extern "C" fn(i32, u64));
    fn rt_popup_show(payload: *const c_char);
    fn rt_popup_update(payload: *const c_char);
    fn rt_popup_hide();
    fn rt_clear_capture();
}
#[cfg(target_os = "macos")]
unsafe fn decode(value: *mut c_char) -> Result<Value, String> {
    if value.is_null() {
        return Err("原生系统调用失败。".into());
    }
    let bytes = CStr::from_ptr(value).to_bytes().to_vec();
    rt_free(value);
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "原生响应格式无效。")?;
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return Err(error.to_string());
    }
    Ok(value)
}
fn c_string(text: &str) -> Result<CString, String> {
    CString::new(text).map_err(|_| "内容包含空字符。".into())
}
pub async fn on_main<T, F>(app: &AppHandle, call: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(call());
    })
    .map_err(|_| "无法访问 macOS 主线程。")?;
    rx.await.map_err(|_| "macOS 系统调用已中断。")?
}
pub fn permission(prompt: bool) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    return Ok(unsafe { rt_permission(i32::from(prompt)) != 0 });
    #[cfg(not(target_os = "macos"))]
    {
        let _ = prompt;
        Err("此版本只支持 macOS。".into())
    }
}
pub fn capture(whole: bool, clipboard: bool) -> Result<Value, String> {
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_capture(i32::from(whole), i32::from(clipboard))) };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (whole, clipboard);
        Err("此版本只支持 macOS。".into())
    }
}
pub fn replace(text: &str) -> Result<Value, String> {
    let text = c_string(text)?;
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_replace(text.as_ptr())) };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("此版本只支持 macOS。".into())
    }
}
pub fn copy(text: &str) -> Result<Value, String> {
    let text = c_string(text)?;
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_copy(text.as_ptr())) };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("此版本只支持 macOS。".into())
    }
}
pub fn key_get(account: &str) -> Result<String, String> {
    let account = c_string(account)?;
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_key_get(account.as_ptr())) }
        .map(|v| v["value"].as_str().unwrap_or("").to_string());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = account;
        Err("此版本只支持 macOS。".into())
    }
}
pub fn key_set(account: &str, key: &str) -> Result<(), String> {
    let account = c_string(account)?;
    let key = c_string(key)?;
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_key_set(account.as_ptr(), key.as_ptr())) }.map(|_| ());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (account, key);
        Err("此版本只支持 macOS。".into())
    }
}
pub fn key_delete(account: &str) -> Result<(), String> {
    let account = c_string(account)?;
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_key_delete(account.as_ptr())) }.map(|_| ());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = account;
        Err("此版本只支持 macOS。".into())
    }
}
pub fn system_proxy() -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_system_proxy()) }.map(|v| v["url"].as_str().map(str::to_string));
    #[cfg(not(target_os = "macos"))]
    Ok(None)
}
pub fn frontmost_bundle() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_frontmost_bundle()) }
        .map(|v| v["bundle"].as_str().unwrap_or("").to_string());
    #[cfg(not(target_os = "macos"))]
    Err("此版本只支持 macOS。".into())
}
pub fn detect_language(text: &str) -> Result<String, String> {
    let text = c_string(text)?;
    #[cfg(target_os = "macos")]
    return unsafe { decode(rt_detect_language(text.as_ptr())) }
        .map(|v| v["language"].as_str().unwrap_or("").to_string());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Ok(String::new())
    }
}
pub fn popup_init(handler: extern "C" fn(i32, u64)) {
    #[cfg(target_os = "macos")]
    unsafe {
        rt_popup_init(handler);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handler;
}
pub fn popup_show(value: &Value) -> Result<(), String> {
    let value = c_string(&value.to_string())?;
    #[cfg(target_os = "macos")]
    unsafe {
        rt_popup_show(value.as_ptr());
    }
    Ok(())
}
pub fn popup_update(value: &Value) -> Result<(), String> {
    let value = c_string(&value.to_string())?;
    #[cfg(target_os = "macos")]
    unsafe {
        rt_popup_update(value.as_ptr());
    }
    Ok(())
}
pub fn popup_hide() {
    #[cfg(target_os = "macos")]
    unsafe {
        rt_popup_hide();
    }
}
pub fn clear_capture() {
    #[cfg(target_os = "macos")]
    unsafe {
        rt_clear_capture();
    }
}
