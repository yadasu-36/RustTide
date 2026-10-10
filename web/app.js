import {languages, initialState, editSource, canReplace, acceptResult, operation} from './model.js';
const $ = id => document.getElementById(id);
const desktop = !!window.__TAURI__;
const invoke = (name, args) => desktop ? window.__TAURI__.core.invoke(name, args) : Promise.reject('请在 RustTide 桌面应用中使用此功能。');
let state = initialState();
let settings;
let configBusy = false;
let manualBusy = false;
let apiTesting = false;
let accessibility = false;
for (const language of languages) $('language').add(new Option(language, language));
function status(message, error=false) {
  $('status').textContent = message;
  $('status').classList.toggle('error', error);
  $('status').hidden = !message;
}
function tab(name) {
  const translate = name === 'translate';
  $('translate-panel').hidden = !translate; $('settings-panel').hidden = translate;
  $('page-title').textContent = translate ? '翻译工作台' : '设置';
  for (const id of ['translate','settings']) {
    $('tab-'+id).classList.toggle('active', id === name);
    if (id === name) $('tab-'+id).setAttribute('aria-current','page');
    else $('tab-'+id).removeAttribute('aria-current');
  }
}
function render() {
  const busy = state.busy || configBusy || manualBusy;
  if ($('source').value !== state.source) $('source').value = state.source;
  if ($('result').value !== state.result) $('result').value = state.result;
  $('source-count').textContent = [...state.source].length.toLocaleString()+' 字';
  $('result-count').textContent = [...state.result].length.toLocaleString()+' 字';
  $('source').disabled = busy; $('result').disabled = busy;
  for (const id of ['translate','capture','language','style','copy','replace','save','test-api']) $(id).disabled = busy;
  $('translate').disabled ||= !state.source.trim();
  $('copy').disabled ||= !state.result.trim();
  $('replace').disabled ||= !canReplace(state);
  $('replace').title = state.captureId === null ? '通过快捷键取词后，才能替换回原输入框' : '替换到 '+state.app;
  $('cancel').hidden = !state.busy;
  $('cancel-api').hidden = !apiTesting;
  $('translate').textContent = state.busy ? '翻译中…' : '翻译';
  $('capture-info').textContent = state.captureId === null ? '手动输入 · 自动识别源语言' : `${state.app} · ${state.whole ? '整个输入框' : '选中文字'}${state.replaceable ? '' : ' · 仅可复制'}`;
  for (const element of $('settings-form').querySelectorAll('input')) element.disabled = busy;
}
function permission(value) {
  accessibility = value;
  $('permission-badge').textContent = value ? '辅助功能已允许' : '需要辅助功能权限';
  $('permission-badge').classList.toggle('allowed', value);
  $('permission-state').textContent = value ? '已允许取词与替换' : '尚未允许取词与替换';
}
async function refreshPermission() {
  try { permission(await invoke('accessibility_status')); } catch (error) { status(String(error), true); }
}
function loadSettings(view) {
  settings = view.settings;
  const fields = {'base-url':'baseUrl','model':'model','timeout':'timeoutSeconds','read-shortcut':'readShortcut','replace-shortcut':'replaceShortcut'};
  for (const [id, key] of Object.entries(fields)) $(id).value = settings[key];
  for (const [id, key] of Object.entries({'clipboard-fallback':'clipboardFallback','whole-input':'captureWholeInput','system-proxy':'useSystemProxy'})) $(id).checked = settings[key];
  $('api-key').value = ''; $('delete-key').checked = false;
  $('key-info').textContent = view.hasKey ? '已保存的 API 地址已有密钥。留空保留原密钥；更换地址时需保存该地址的密钥。' : '已保存的 API 地址尚无密钥。保存后写入 macOS 钥匙串，本机 API 可不填。';
  $('language').value = settings.targetLanguage; $('style').value = settings.style;
  const hint = shortcut => shortcut.replace(/Alt/gi,'⌥').replace(/Command/gi,'⌘').replace(/Shift/gi,'⇧').replaceAll('+',' ');
  $('read-hint').textContent = hint(settings.readShortcut); $('replace-hint').textContent = hint(settings.replaceShortcut);
  permission(view.accessibility);
  if (view.startupError) status(view.startupError, true);
}
$('tab-translate').addEventListener('click', () => tab('translate'));
$('tab-settings').addEventListener('click', () => tab('settings'));
$('permission-badge').addEventListener('click', () => tab('settings'));
$('source').addEventListener('input', event => { state = editSource(state, event.target.value); render(); });
$('result').addEventListener('input', event => { state = {...state, result:event.target.value}; render(); });
$('translate').addEventListener('click', async () => {
  state = {...state, busy:true, result:'', revision:state.revision+1};
  const revision = state.revision;
  status('正在翻译…'); render();
  try {
    const value = await invoke('translate_text', {text:state.source, language:$('language').value, style:$('style').value, captureId:state.captureId});
    state = acceptResult(state, revision, value.result); status('翻译完成。');
  } catch (error) { status(String(error), String(error) !== '已取消。'); }
  finally { state = {...state, busy:false}; render(); }
});
$('capture').addEventListener('click', async () => {
  try { await invoke('capture_later'); } catch (error) { status(String(error), true); }
});
async function cancel() {
  status('正在取消…');
  try { await invoke('cancel_operation'); } catch (error) { status(String(error),true); }
}
$('cancel').addEventListener('click', cancel);
$('cancel-api').addEventListener('click', cancel);
$('copy').addEventListener('click', async () => {
  manualBusy = true; render();
  try { await invoke('copy_text', {text:state.result}); status('译文已复制。'); }
  catch (error) { status(String(error),true); }
  finally { manualBusy = false; render(); }
});
$('replace').addEventListener('click', async () => {
  manualBusy = true; render();
  try {
    const value = await invoke('replace_result', {captureId:state.captureId, text:state.result});
    state = operation(state, {kind:'replaced'});
    status(value.verified ? '已替换原文，可在原应用中按 ⌘Z 撤销。' : '已发送粘贴，请在原应用中确认结果；可按 ⌘Z 撤销。');
  } catch (error) { status(String(error),true); }
  finally { manualBusy = false; render(); }
});
$('settings-form').addEventListener('submit', async event => {
  event.preventDefault();
  const next = {...settings, baseUrl:$('base-url').value, model:$('model').value, timeoutSeconds:Number($('timeout').value),
    readShortcut:$('read-shortcut').value.trim(), replaceShortcut:$('replace-shortcut').value.trim(),
    targetLanguage:$('language').value, style:$('style').value, clipboardFallback:$('clipboard-fallback').checked,
    captureWholeInput:$('whole-input').checked, useSystemProxy:$('system-proxy').checked};
  const apiKey = $('api-key').value || null;
  // Clear the password field immediately; it is never returned by Rust or saved in web storage.
  $('api-key').value = '';
  configBusy = true; render(); status('正在保存设置…');
  try {
    await invoke('save_settings', {settings:next, apiKey, deleteKey:$('delete-key').checked});
    loadSettings(await invoke('get_settings')); status('设置已保存。');
  } catch (error) { status(String(error),true); }
  finally { configBusy = false; render(); }
});
$('test-api').addEventListener('click', async () => {
  configBusy = true; apiTesting = true; render(); status('正在使用已保存的配置翻译示例文字…');
  try { status('API 测试成功：'+await invoke('test_api')); }
  catch (error) { status(String(error),true); }
  finally { configBusy = false; apiTesting = false; render(); }
});
$('grant-permission').addEventListener('click', async () => {
  try { permission(await invoke('request_permission')); if (!accessibility) status('请在系统设置中允许 RustTide，然后重新检查权限。'); }
  catch (error) { status(String(error),true); }
});
$('refresh-permission').addEventListener('click',refreshPermission);
window.addEventListener('focus', () => { if (desktop) refreshPermission(); });
async function initialize() {
  render();
  if (!desktop) { status('界面预览：请运行 RustTide 桌面应用以使用取词和翻译。'); permission(false); return; }
  await window.__TAURI__.event.listen('operation', ({payload}) => {
    state = operation(state, payload);
    if (payload.kind === 'started') status(payload.delayed ? '3 秒后取词，请切换到目标应用并选中文字…' : '正在取词和翻译…');
    if (payload.kind === 'captured') { tab('translate'); status('已取词，正在翻译…'); }
    if (payload.kind === 'translated') status(payload.direct ? '正在替换原文…' : '翻译完成。');
    if (payload.kind === 'replaced') status(payload.verified ? '已替换原文，可在原应用中按 ⌘Z 撤销。' : '已发送粘贴，请在原应用中确认结果。');
    if (payload.kind === 'error') status(payload.message, payload.message !== '已取消。');
    render();
  });
  try {
    const view = await invoke('get_settings'); loadSettings(view);
    if (!view.hasKey && view.settings.baseUrl.startsWith('https:')) tab('settings');
  } catch (error) { status(String(error),true); }
}
initialize().catch(error => status(String(error),true));
