import test from 'node:test';
import assert from 'node:assert/strict';
import {fields, settingsFromForm, shortcutLabel} from './model.js';
test('saving input preferences preserves the API and independent shortcut actions', () => {
  const current = {baseUrl:'https://api.example.com/v1', model:'custom', timeoutSeconds:45,
    nativeLanguage:'简体中文', foreignLanguage:'English', style:'formal', readShortcut:'Alt+D', replaceShortcut:'Alt+R',
    readDirection:'native', writeDirection:'foreign', readOutput:'popup', writeOutput:'replace'};
  const values = Object.fromEntries(Object.entries(fields).map(([id,key]) => [id,current[key]]));
  Object.assign(values, {'write-output':'popup','clipboard-fallback':true,'system-proxy':false,'excluded-apps':' com.example.Editor\n\ncom.example.Passwords '});
  const next = settingsFromForm(current, values);
  assert.equal(next.baseUrl, current.baseUrl);
  assert.equal(next.readOutput, 'popup');
  assert.equal(next.writeOutput, 'popup');
  assert.equal(next.writeDirection, 'foreign');
  assert.deepEqual(next.excludedApps, ['com.example.Editor','com.example.Passwords']);
  assert.equal(next.timeoutSeconds,45);
  assert.equal(next.clipboardFallback,true);
  assert.equal(next.useSystemProxy,false);
  assert.equal(next.apiKey,undefined);
});
test('custom shortcuts render the actual saved macOS combinations', () => {
  assert.equal(shortcutLabel('Command+Shift+T'),'⌘ ⇧ T');
  assert.equal(shortcutLabel('Alt+R'),'⌥ R');
});
