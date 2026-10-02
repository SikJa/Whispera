// Run against a local test build launched with WebView2 CDP on 9223.
// Uses real start/stop hotkeys and a cached fixture transcript, with no API request.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync} from 'node:child_process';
import {DatabaseSync} from 'node:sqlite';
import {chromium} from 'playwright';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9223'),context=browser.contexts()[0];
const invoke=(p,cmd,args={})=>p.evaluate(([cmd,args])=>window.__TAURI_INTERNALS__.invoke(cmd,args),[cmd,args]);
async function windowFor(label){for(let i=0;i<100;i++){for(const p of context.pages())if(await p.evaluate(()=>window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label).catch(()=>null)===label)return p;await new Promise(r=>setTimeout(r,50));}throw Error(label);}
async function wait(check,description){for(let i=0;i<200;i++){if(await check())return;await new Promise(r=>setTimeout(r,100));}throw Error(description);}
function pressDictation(focusSettings=false){
  const source='using System; using System.Runtime.InteropServices; public static class WhisperaDictationKeys { [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(IntPtr cls,string title); [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd,int command); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd); [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); }';
  const focus=focusSettings?'$window=[WhisperaDictationKeys]::FindWindow([IntPtr]::Zero,"Whispera 2 - Configuracion"); if ($window -eq [IntPtr]::Zero) { throw "Test window missing" }; [void][WhisperaDictationKeys]::ShowWindow($window,9); [void][WhisperaDictationKeys]::SetForegroundWindow($window); Start-Sleep -Milliseconds 100; if ([WhisperaDictationKeys]::GetForegroundWindow() -ne $window) { throw "Test window is not foreground" };':'';
  execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',`Add-Type -TypeDefinition '${source}'\n${focus}\n[WhisperaDictationKeys]::keybd_event(18,0,0,[UIntPtr]::Zero)\n[WhisperaDictationKeys]::keybd_event(90,0,0,[UIntPtr]::Zero)\nStart-Sleep -Milliseconds 40\n[WhisperaDictationKeys]::keybd_event(90,0,2,[UIntPtr]::Zero)\n[WhisperaDictationKeys]::keybd_event(18,0,2,[UIntPtr]::Zero)`],{windowsHide:true,timeout:10000,stdio:'pipe'});
}
function clipboardText(){
  const raw=execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command','[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); Get-Clipboard -Raw | ConvertTo-Json -Compress'],{windowsHide:true,timeout:10000,encoding:'utf8'}).trim();
  return raw?JSON.parse(raw):null;
}
const main=await windowFor('main'),root=path.join(process.env.APPDATA,'app.whispera.desktop'),ids=[];
const manifest=path.resolve('../../.local/dictation-check.json');
const earlier=fs.existsSync(manifest)?JSON.parse(fs.readFileSync(manifest,'utf8')).sessions:[];
const previousClipboard=clipboardText();let lastText='';
try {
  assert.equal((await invoke(main,'screen_status')).phase,'idle');
  assert.ok(!['recording','paused','processing'].includes((await invoke(main,'recording_state')).phase));
  const settings=await invoke(main,'read_settings');assert.equal(settings.hotkey.toLowerCase(),'alt+z');assert.equal(settings.autoPaste,true);
  for(let cycle=0;cycle<2;cycle++){
    await invoke(main,'open_settings');
    pressDictation(true);await wait(async()=>(await invoke(main,'recording_state')).phase==='recording','dictation start');
    const recording=await invoke(main,'recording_state');assert.match(recording.session,/^[a-f0-9-]{36}$/);
    ids.push(recording.session);fs.writeFileSync(manifest,JSON.stringify({sessions:[...earlier,...ids]},null,2));
    const dir=path.join(root,'recordings',recording.session);
    lastText=`Prueba local de cierre automatico ${cycle+1}.`;
    fs.writeFileSync(path.join(dir,'part-0000.txt'),lastText,{flag:'wx'});
    const recorder=await windowFor('recorder');
    await wait(async()=>await invoke(recorder,'plugin:window|is_visible',{label:'recorder'}),'recorder visible');
    await wait(async()=>(await invoke(main,'recording_state')).seconds>=1,'audio captured');
    pressDictation();
    await wait(async()=>{
      const state=await invoke(main,'recording_state');if(state.phase==='error')throw Error(state.error);
      return state.phase==='done'&&state.session===recording.session;
    },'dictation completion');
    const done=await invoke(main,'recording_state');assert.equal(done.text,lastText);
    assert.match(done.error,/No habia un campo de destino/,'reproduces the reported paste warning');
    assert.equal(clipboardText(),lastText,'completed transcript remains copied');
    assert.ok(fs.existsSync(path.join(dir,'completed.json')));
    await wait(async()=>!await invoke(recorder,'plugin:window|is_visible',{label:'recorder'}),'recorder automatically hidden');
    await new Promise(r=>setTimeout(r,600));
    assert.equal(await invoke(recorder,'plugin:window|is_visible',{label:'recorder'}),false,'polling does not reopen the folder');
  }
  console.log('PASS: two real Alt+Z start/stop cycles, transcript saved/copied, exact no-destination warning reproduced, folder hides and stays hidden; cached test text avoids external transcription requests.');
} finally {
  const state=await invoke(main,'recording_state').catch(()=>null);
  if(state&&ids.includes(state.session)&&['recording','paused'].includes(state.phase))await invoke(main,'recording_action',{action:'cancel'}).catch(()=>{});
  if(typeof previousClipboard==='string'&&clipboardText()===lastText)await invoke(main,'copy_text',{text:previousClipboard});
  const db=new DatabaseSync(path.join(root,'whispera.sqlite'));
  for(const id of ids)db.prepare('DELETE FROM history WHERE id=?').run(id);
  db.close();await browser.close();
}
