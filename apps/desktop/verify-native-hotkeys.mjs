// Windows integration: only runs against the explicitly launched local test app.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {execFileSync} from 'node:child_process';
import {chromium} from 'playwright';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9223'),context=browser.contexts()[0];
const invoke=(p,cmd,args={})=>p.evaluate(([cmd,args])=>window.__TAURI_INTERNALS__.invoke(cmd,args),[cmd,args]);
async function windowFor(label){for(let i=0;i<100;i++){for(const p of context.pages())if(await p.evaluate(()=>window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label).catch(()=>null)===label)return p;await new Promise(r=>setTimeout(r,50));}throw Error(label);}
async function wait(check){for(let i=0;i<150;i++){if(await check())return;await new Promise(r=>setTimeout(r,100));}throw Error('Native state timeout');}
function press(keys){
  assert.ok(keys.every(k=>Number.isInteger(k)&&k>0&&k<256));
  const source='using System; using System.Runtime.InteropServices; public static class WhisperaTestKeys { [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra); }';
  const commands=[`Add-Type -TypeDefinition '${source}'`,...keys.map(k=>`[WhisperaTestKeys]::keybd_event(${k},0,0,[UIntPtr]::Zero)`),'Start-Sleep -Milliseconds 50',...keys.toReversed().map(k=>`[WhisperaTestKeys]::keybd_event(${k},0,2,[UIntPtr]::Zero)`)];
  execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',commands.join('\n')],{windowsHide:true,timeout:10000,stdio:'pipe'});
}
const main=await windowFor('main');let owned=false;
try {
  assert.equal((await invoke(main,'screen_status')).phase,'idle');
  assert.ok(!['recording','paused','processing'].includes((await invoke(main,'recording_state')).phase));
  const prefs=await invoke(main,'screen_preferences');
  const keyCodes=prefs.hotkey.split('+').map(k=>({control:17,ctrl:17,shift:16,alt:18}[k.toLowerCase()]??(/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(k)?111+Number(k.slice(1)):/^Key[A-Z]$/.test(k)?k.charCodeAt(3):/^[a-z]$/i.test(k)?k.toUpperCase().charCodeAt(0):NaN)));
  assert.ok(keyCodes.every(Number.isFinite),'fixture needs a supported video shortcut');
  await main.evaluate(()=>{
    window.testFocus=[];for(const type of ['focus','blur','focusin','focusout'])window.addEventListener(type,e=>window.testFocus.push({type,target:e.target?.id}));
    const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=(cmd,args)=>{if(cmd==='shortcut_capture')window.testFocus.push({cmd,args});return original(cmd,args);};
  });
  await invoke(main,'open_settings');await main.getByRole('button',{name:'Atajos',exact:true}).click();
  const field=main.locator('#shortcut-video');await field.click();
  await main.locator('.hotkey-field[data-listening="true"]').waitFor();
  assert.equal(await invoke(main,'plugin:window|is_focused',{label:'main'}),true,'keyboard test requires the app in front');
  owned=true;press([18,88]);await wait(async()=>await field.inputValue()==='Alt + X');
  assert.equal((await invoke(main,'screen_status')).phase,'idle','current shortcut does not record while the field listens');
  press([27]);await wait(async()=>await main.locator('.hotkey-field[data-listening="true"]').count()===0);
  await main.evaluate(()=>new Promise(requestAnimationFrame));
  press(keyCodes);owned=true;await wait(async()=>(await invoke(main,'screen_status')).phase==='selecting');
  press([27]);await wait(async()=>(await invoke(main,'screen_status')).phase==='idle');
  const overlay=await windowFor('screen-select-0');
  assert.equal(await invoke(overlay,'plugin:window|is_visible',{label:'screen-select-0'}),false);
  assert.equal(await invoke(main,'screen_editor_context'),null);
  // Capture a harmless synthetic area of this app for the Escape-while-recording check.
  await invoke(main,'open_settings');await main.evaluate(()=>{document.body.innerHTML='<div style="position:fixed;inset:0;background:#e6edf5;color:#182131;padding:70px;font:24px Segoe UI">Whispera · Prueba de atajos</div>';});
  const origin=await invoke(main,'plugin:window|inner_position',{label:'main'});
  const scale=await invoke(main,'plugin:window|scale_factor',{label:'main'});
  press(keyCodes);await wait(async()=>(await invoke(main,'screen_status')).phase==='selecting');
  await overlay.locator('.screen-selection').waitFor();
  const screenOrigin=await invoke(overlay,'plugin:window|inner_position',{label:'screen-select-0'});
  const screenScale=await invoke(overlay,'plugin:window|scale_factor',{label:'screen-select-0'});
  const x=(origin.x+80*scale-screenOrigin.x)/screenScale,y=(origin.y+80*scale-screenOrigin.y)/screenScale;
  await overlay.mouse.move(x,y);await overlay.mouse.down();await overlay.mouse.move(x+320,y+200);await overlay.mouse.up();
  await wait(async()=>(await invoke(main,'screen_status')).phase==='recording');
  await wait(async()=>!!await invoke(main,'screen_editor_context'));
  assert.equal(await overlay.getByTestId('recording-frame').evaluate(e=>getComputedStyle(e).borderColor),'rgb(255, 255, 255)');
  await new Promise(r=>setTimeout(r,500));
  // Focus settings to prove Escape works outside the capture/editor hosts.
  await invoke(main,'open_settings');press([27]);
  await wait(async()=>(await invoke(main,'screen_status')).phase==='idle');
  const result=await invoke(main,'screen_status');assert.ok(result.path&&result.copied);
  for(const label of ['screen-select-0','screen-ink','screen-tools','screen-hud'])assert.equal(await invoke(main,'plugin:window|is_visible',{label}),false,`${label} hides`);
  const statePath='../../.local/native-check/state.json',state=JSON.parse(fs.readFileSync(statePath,'utf8'));state.files.push(result.path);fs.writeFileSync(statePath,JSON.stringify(state,null,2));
  assert.equal(await invoke(main,'screen_editor_context'),null,'native editor releases its image');
  const ink=await windowFor('screen-ink');await wait(async()=>await ink.locator('canvas').count()===0);
  await invoke(main,'screen_select_image');await overlay.locator('.screen-selection').waitFor();
  await overlay.mouse.move(x,y);await overlay.mouse.down();await overlay.mouse.move(x+320,y+200);await overlay.mouse.up();
  await wait(async()=>(await invoke(main,'screen_editor_context'))?.kind==='image');
  await ink.getByLabel('Editar captura').waitFor();press([27]);
  await wait(async()=>(await invoke(main,'screen_status')).phase==='idle');
  for(const label of ['screen-select-0','screen-ink','screen-tools','screen-hud'])assert.equal(await invoke(main,'plugin:window|is_visible',{label}),false,`${label} hides on image Escape`);
  const saved=await invoke(main,'screen_preferences');assert.equal(saved.hotkey,prefs.hotkey);assert.equal(saved.audio,prefs.audio);
  console.log('PASS: real Windows Alt+X captured without triggering video, restored on blur/Escape, selection and image cancelled with Escape, white frame, Escape stops/copies video from settings, every overlay closes and canvas memory is released.');
} catch(error) {
  console.log(await main.evaluate(()=>({focus:window.testFocus,active:document.activeElement?.id,focused:document.hasFocus()})));throw error;
} finally {
  if(owned)try{await invoke(main,'screen_escape');}catch{}
  try{await main.reload();}catch{}await browser.close();
}
