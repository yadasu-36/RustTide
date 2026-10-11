export const languages = ['English', '简体中文', '繁體中文', '日本語', '한국어', 'Español', 'Français', 'Deutsch', 'Português', 'Русский', 'العربية', 'हिन्दी'];
export const fields = {
  'base-url':'baseUrl', model:'model', timeout:'timeoutSeconds',
  'native-language':'nativeLanguage', 'foreign-language':'foreignLanguage', style:'style',
  'read-shortcut':'readShortcut', 'write-shortcut':'replaceShortcut',
  'read-direction':'readDirection', 'write-direction':'writeDirection',
  'read-output':'readOutput', 'write-output':'writeOutput',
};
export const checks = {'clipboard-fallback':'clipboardFallback','system-proxy':'useSystemProxy'};
export function settingsFromForm(current, values) {
  const next = {...current};
  for (const [id, key] of Object.entries(fields)) next[key] = values[id];
  next.timeoutSeconds = Number(values.timeout);
  next.readShortcut = next.readShortcut.trim();
  next.replaceShortcut = next.replaceShortcut.trim();
  for (const [id, key] of Object.entries(checks)) next[key] = !!values[id];
  next.excludedApps = values['excluded-apps'].split('\n').map(id => id.trim()).filter(Boolean);
  return next;
}
export function shortcutLabel(shortcut) {
  return shortcut.replace(/Alt/gi,'⌥').replace(/Command|Super/gi,'⌘').replace(/Shift/gi,'⇧').replace(/Control|Ctrl/gi,'⌃').replaceAll('+',' ');
}
