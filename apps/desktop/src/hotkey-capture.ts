import { invoke } from '@tauri-apps/api/core';

// Serialize focus changes so moving between fields cannot leave shortcuts paused.
let pending: Promise<unknown> = Promise.resolve();
export function captureHotkey(active: boolean) {
  if (!('__TAURI_INTERNALS__' in window)) return Promise.resolve();
  pending = pending.catch(() => {}).then(() => invoke('shortcut_capture', { active }));
  return pending;
}
export async function finishHotkeyCapture() { await captureHotkey(false); }

const names: Record<string,string> = {
  control:'Ctrl', ctrl:'Ctrl', alt:'Alt', shift:'Shift', super:'Win', meta:'Win',
  space:'Espacio', enter:'Enter', escape:'Esc', backspace:'Retroceso', delete:'Supr',
  arrowup:'↑', arrowdown:'↓', arrowleft:'←', arrowright:'→',
  equal:'=', minus:'−', comma:',', period:'.', slash:'/', backslash:'\\',
  semicolon:';', quote:"'", backquote:'`', bracketleft:'[', bracketright:']',
  printscreen:'Impr Pant',
};
export function formatHotkey(value: string) {
  return value.split('+').map(part => names[part.toLowerCase()] ?? part.replace(/^Key|^Digit/i,'').toUpperCase()).join(' + ');
}
export function modifiers(e: Pick<KeyboardEvent,'ctrlKey'|'altKey'|'shiftKey'|'metaKey'>) {
  return [e.ctrlKey&&'Control',e.altKey&&'Alt',e.shiftKey&&'Shift',e.metaKey&&'Super'].filter(Boolean) as string[];
}
export function eventHotkey(e: KeyboardEvent): string | undefined {
  if (e.isComposing || e.repeat || e.getModifierState('AltGraph')) return;
  // DOM physical codes match the native shortcut parser, including non-US keyboards.
  let code=e.code;
  // Virtual/accessibility keyboards may omit the scan code. Keep physical codes
  // authoritative and only recover keys with an unambiguous native equivalent.
  if (!code || code === 'Unidentified') {
    if (/^[a-z]$/i.test(e.key)) code=`Key${e.key.toUpperCase()}`;
    else if (/^[0-9]$/.test(e.key)) code=`Digit${e.key}`;
    else if (e.key === ' ') code='Space';
    else if (/^(F([1-9]|1[0-9]|2[0-4])|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right)|CapsLock|NumLock|ScrollLock|PrintScreen|Pause)$/.test(e.key)) code=e.key;
  }
  if (!/^(Key[A-Z]|Digit[0-9]|F([1-9]|1[0-9]|2[0-4])|Numpad([0-9]|Add|Subtract|Multiply|Divide|Decimal|Enter|Equal)|Space|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right)|Backquote|Backslash|Bracket(Left|Right)|Comma|Equal|Minus|Period|Quote|Semicolon|Slash|CapsLock|NumLock|ScrollLock|PrintScreen|Pause)$/.test(code)) return;
  return [...modifiers(e),code].join('+');
}
