import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';

const source = fs.readFileSync(new URL('./src/hotkey-capture.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {compilerOptions: {module: ts.ModuleKind.CommonJS}}).outputText;
function captureFor(isMac) {
  const exports = {};
  vm.runInNewContext(compiled, {exports, require(name) {
    if (name === './platform') return {isMac};
    if (name === '@tauri-apps/api/core') return {invoke: async () => {}};
    throw Error(name);
  }});
  return exports.eventHotkey;
}
const event = (key, code, extra = {}) => ({key, code, isComposing:false, repeat:false,
  ctrlKey:false, altKey:false, shiftKey:false, metaKey:false, getModifierState:() => false, ...extra});
const windows = captureFor(false);
assert.equal(windows(event('|', 'Backquote')), '|', 'Latin American key below Escape');
assert.equal(windows(event('ñ', 'Semicolon')), 'ñ', 'Spanish printable OEM key');
assert.equal(windows(event('`', 'Backquote')), '`', 'US keyboard remains supported');
assert.equal(windows(event('|', '')), '|', 'Printable key without a physical code');
assert.equal(windows(event('a', 'KeyA')), 'a');
assert.equal(windows(event(' ', 'Space')), 'Space', 'Space retains its visible label');
assert.equal(windows(event('a', 'KeyA', {ctrlKey:true})), 'Control+KeyA');
assert.equal(windows(event('PrintScreen', 'PrintScreen')), 'PrintScreen');
assert.equal(windows(event('|', 'Backquote', {repeat:true})), undefined);
assert.equal(windows(event('|', 'Backquote', {isComposing:true})), undefined);
assert.equal(windows(event('|', 'Backquote', {getModifierState:() => true})), undefined);
assert.equal(windows(event('Dead', 'Backquote')), 'Backquote');
assert.equal(captureFor(true)(event('|', 'Backquote')), 'Backquote', 'macOS mapping is unchanged');
console.log('PASS: Windows printable layout keys, named shortcuts, modifiers, IME/AltGr/repeat, and macOS.');
