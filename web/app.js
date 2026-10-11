import {languages, fields, checks, settingsFromForm, shortcutLabel} from './model.js';
const $ = id => document.getElementById(id);
const desktop = !!window.__TAURI__;
const invoke = (name, args) => desktop ? window.__TAURI__.core.invoke(name, args) : Promise.reject('请在 RustTide 桌面应用中使用此功能。');
let settings;
let working = false;
let operationBusy = false;
let testing = false;
let dirty = false;
for (const id of ['native-language','foreign-language']) {
  for (const language of languages) $(id).add(new Option(language, language));
}
function status(message, error = false) {
  $('status').textContent = message;
  $('status').classList.toggle('error', error);
  $('status').hidden = !message;
}
function tab(name) {
  const titles = {input:'输入与翻译',api:'API 服务',system:'权限与隐私'};
  for (const id of Object.keys(titles)) {
    $(id+'-panel').hidden = id !== name;
    $('tab-'+id).classList.toggle('active', id === name);
    if (id === name) $('tab-'+id).setAttribute('aria-current','page');
    else $('tab-'+id).removeAttribute('aria-current');
  }
  $('page-title').textContent = titles[name];
}
for (const name of ['input','api','system']) $('tab-'+name).addEventListener('click', () => tab(name));
$('permission-badge').addEventListener('click', () => tab('system'));
function render() {
  const busy = working || operationBusy;
  for (const field of $('settings-form').querySelectorAll('input,select,textarea')) field.disabled = busy;
  $('save').disabled = busy || !settings;
  $('test-api').disabled = busy || !settings || dirty;
  $('test-api').title = dirty ? '先保存修改，再测试服务' : '只使用已保存的配置';
  $('cancel-api').hidden = !testing;
  $('save-hint').textContent = operationBusy ? '当前输入正在翻译，可在浮窗或菜单栏取消。' : dirty ? '有未保存的设置。' : '关闭设置后，快捷键仍可使用。';
}
function permission(value) {
  $('permission-badge').textContent = value ? '辅助功能已允许' : '需要辅助功能权限';
  $('permission-badge').classList.toggle('allowed', value);
  $('permission-state').textContent = value ? '已允许读取与替换' : '尚未允许读取与替换';
}
async function refreshPermission() {
  try { permission(await invoke('accessibility_status')); }
  catch (error) { status(String(error), true); }
}
function loadSettings(view) {
  settings = view.settings;
  for (const [id,key] of Object.entries(fields)) $(id).value = settings[key];
  for (const [id,key] of Object.entries(checks)) $(id).checked = settings[key];
  $('excluded-apps').value = settings.excludedApps.join('\n');
  $('api-key').value = ''; $('delete-key').checked = false;
  $('key-info').textContent = view.hasKey ? '此地址已有密钥，留空保留；更换 API 地址后需保存新地址的密钥。' : '密钥保存到 macOS 钥匙串。本机 API 可以不填。';
  $('read-hint').textContent = shortcutLabel(settings.readShortcut);
  $('write-hint').textContent = shortcutLabel(settings.replaceShortcut);
  $('native-guide').textContent = settings.nativeLanguage;
  const target = settings.writeDirection === 'native' ? settings.nativeLanguage : settings.foreignLanguage;
  $('write-effect').textContent = settings.writeOutput === 'popup' ? `在浮窗预览 ${target} 译文` : `原地译为 ${target}`;
  $('read-effect').textContent = settings.readOutput === 'popup' ? '译文在选区旁显示' : '翻译并替换可写输入框';
  dirty = false;
  permission(view.accessibility);
  if (view.startupError) status(view.startupError, true);
}
$('settings-form').addEventListener('input', () => { dirty = true; render(); });
$('settings-form').addEventListener('change', () => { dirty = true; render(); });
$('settings-form').addEventListener('submit', async event => {
  event.preventDefault();
  if (!settings || working || operationBusy) return;
  const values = {};
  for (const id of Object.keys(fields)) values[id] = $(id).value;
  for (const id of Object.keys(checks)) values[id] = $(id).checked;
  values['excluded-apps'] = $('excluded-apps').value;
  const next = settingsFromForm(settings, values);
  if (!next.baseUrl.trim() || !next.model.trim() || !Number.isFinite(next.timeoutSeconds)) {
    tab('api'); status('请填写 API 地址、模型和超时。', true); return;
  }
  if (next.nativeLanguage === next.foreignLanguage) {
    tab('input'); status('母语和外语需要选择不同的语言。', true); $('foreign-language').focus(); return;
  }
  const apiKey = $('api-key').value || null;
  $('api-key').value = '';
  working = true; render(); status('正在保存…');
  try {
    await invoke('save_settings', {settings:next, apiKey, deleteKey:$('delete-key').checked});
    loadSettings(await invoke('get_settings')); status('已保存。关闭设置，在原输入框中使用快捷键即可。');
  } catch (error) { status(String(error), true); }
  finally { working = false; render(); }
});
$('test-api').addEventListener('click', async () => {
  working = true; testing = true; render(); status('正在翻译固定示例…');
  try { status('API 可用：'+await invoke('test_api')); }
  catch (error) { status(String(error), String(error) !== '已取消。'); }
  finally { working = false; testing = false; render(); }
});
$('cancel-api').addEventListener('click', async () => {
  // Set the pending state before IPC, so its acknowledgement cannot overwrite
  // the translation task's completed cancellation state.
  status('正在取消…');
  try { await invoke('cancel_operation'); }
  catch (error) { status(String(error), true); }
});
$('grant-permission').addEventListener('click', async () => {
  try {
    const allowed = await invoke('request_permission'); permission(allowed);
    if (!allowed) status('在系统设置中允许 RustTide，然后重新检查。');
  } catch (error) { status(String(error), true); }
});
$('refresh-permission').addEventListener('click', refreshPermission);
window.addEventListener('focus', () => { if (desktop) refreshPermission(); });
async function initialize() {
  render();
  if (!desktop) { permission(false); status('设置界面预览。运行桌面应用后，使用全局快捷键翻译。'); return; }
  await window.__TAURI__.event.listen('operation', ({payload}) => { operationBusy = payload.busy; render(); });
  const view = await invoke('get_settings');
  loadSettings(view); render();
  if (!view.hasKey && view.settings.baseUrl.startsWith('https:')) tab('api');
}
initialize().catch(error => status(String(error), true));
