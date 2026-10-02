// Windows integration against an explicitly launched local build with CDP on 9223.
// Only records synthetic colors inside Whispera. User preferences are restored.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {chromium} from 'playwright';
import {snapshotCaptureHistory,restoreCaptureHistory} from './tests/native-history.mjs';
const browser=await chromium.connectOverCDP('http://127.0.0.1:9223'),context=browser.contexts()[0];
const invoke=(p,command,args={})=>p.evaluate(([command,args])=>window.__TAURI_INTERNALS__.invoke(command,args),[command,args]);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(check,label){for(let i=0;i<200;i++){if(await check())return;await sleep(100);}throw Error(`Timeout: ${label}`);}
async function windowFor(label){let result;await wait(async()=>{for(const p of context.pages())if(await p.evaluate(()=>window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label).catch(()=>null)===label){result=p;return true;}},label);return result;}
const main=await windowFor('main'),output=path.resolve('../../.local/native-workflow');fs.mkdirSync(output,{recursive:true});
const preferences=await invoke(main,'screen_preferences'),settings=await invoke(main,'read_settings');
const encoder=path.resolve('node_modules/ffmpeg-static/ffmpeg.exe');
const audio=process.env.WHISPERA_TEST_AUDIO??'none';assert.ok(['none','system','microphone','both'].includes(audio));
const files=[];let owned=false;
const historyBefore=snapshotCaptureHistory();
assert.equal((await invoke(main,'screen_status')).phase,'idle','requires idle capture');
assert.ok(!['recording','paused','processing'].includes((await invoke(main,'recording_state')).phase),'requires idle voice');
try {
  await invoke(main,'screen_save_preferences',{preferences:{...preferences,audio,image_auto_copy:false}});
  owned=true;
  // Reject duplicate hotkeys without losing any previously working shortcut.
  await assert.rejects(invoke(main,'save_all_shortcuts',{voice:settings.hotkey,video:settings.hotkey,image:preferences.image_hotkey}));
  assert.equal((await invoke(main,'read_settings')).hotkey,settings.hotkey);
  assert.equal((await invoke(main,'screen_preferences')).hotkey,preferences.hotkey);
  // A valid swap between the two screen functions must work atomically.
  await invoke(main,'save_all_shortcuts',{voice:settings.hotkey,video:preferences.image_hotkey,image:preferences.hotkey});
  assert.equal((await invoke(main,'screen_preferences')).hotkey,preferences.image_hotkey);
  await invoke(main,'save_all_shortcuts',{voice:settings.hotkey,video:preferences.hotkey,image:preferences.image_hotkey});
  await invoke(main,'open_settings');
  const position=await invoke(main,'plugin:window|inner_position',{label:'main'});
  const scale=await invoke(main,'plugin:window|scale_factor',{label:'main'});
  await main.evaluate(()=>{document.body.innerHTML='<div id="pattern" style="position:fixed;inset:0;background:#1267dc"></div>';});
  const color=async value=>{await main.locator('#pattern').evaluate((el,value)=>el.style.background=value,value);await main.evaluate(()=>new Promise(requestAnimationFrame).then(()=>new Promise(requestAnimationFrame)));};
  async function select(kind){
    await invoke(main,kind==='image'?'screen_select_image':'screen_select');
    const overlay=await windowFor('screen-select-0');await overlay.locator('.screen-selection').waitFor();
    await wait(async()=>await invoke(overlay,'plugin:window|is_visible',{label:'screen-select-0'})&&await invoke(overlay,'plugin:window|is_focused',{label:'screen-select-0'}),'selector visible and focused');
    const origin=await invoke(overlay,'plugin:window|inner_position',{label:'screen-select-0'}),s=await invoke(overlay,'plugin:window|scale_factor',{label:'screen-select-0'});
    const x=(position.x+90*scale-origin.x)/s,y=(position.y+90*scale-origin.y)/s;
    // Drive the native selection result directly for this motor test. Pointer
    // release is exercised separately by verify-native-screen and verify-screen.
    await invoke(overlay,'screen_start',{rect:{x,y,width:320*scale/s,height:200*scale/s}});
    return overlay;
  }
  await color('#1267dc');
  const overlay=await select('video');
  await wait(async()=>(await invoke(main,'screen_status')).phase==='recording','recording');
  const hud=await windowFor('screen-hud'),tools=await windowFor('screen-tools');
  await hud.getByRole('button',{name:'Pausar video'}).waitFor();
  assert.equal(await tools.locator('.capture-footer').count(),0,'no video footer');
  await sleep(850);
  await hud.getByRole('button',{name:'Pausar video'}).evaluate(el=>el.click());
  await wait(async()=>(await invoke(main,'screen_status')).phase==='paused','paused');
  const paused=await invoke(main,'screen_status');
  await color('#ed172f');await sleep(1700);
  assert.equal((await invoke(main,'screen_status')).seconds,paused.seconds,'timer freezes in pause');
  await color('#16c97a');
  await hud.getByRole('button',{name:'Reanudar video'}).evaluate(el=>el.click());
  await wait(async()=>(await invoke(main,'screen_status')).phase==='recording','resumed');
  await sleep(900);
  const compact=await hud.locator('.capture-hud').getAttribute('data-compact');
  await hud.getByRole('button',{name:compact==='true'?'Mostrar carpeta':'Ocultar carpeta'}).evaluate(el=>el.click());
  await wait(async()=>await hud.locator('.capture-hud').getAttribute('data-compact')!==compact,'compact toggle');
  await hud.getByRole('button',{name:compact==='true'?'Ocultar carpeta':'Mostrar carpeta'}).evaluate(el=>el.click());
  await tools.getByRole('button',{name:'Lápiz',exact:true}).evaluate(el=>el.click());
  await tools.getByRole('button',{name:'Contraer herramientas'}).evaluate(el=>el.click());
  await wait(async()=>(await invoke(main,'screen_editor_feedback_get')).tool==='pointer','collapse restores mouse');
  assert.equal((await invoke(tools,'plugin:window|inner_size',{label:'screen-tools'})).height,Math.round(46*scale));
  await invoke(main,'screen_pause');
  await wait(async()=>(await invoke(main,'screen_status')).phase==='paused','second pause');
  await invoke(main,'screen_escape');
  const video=await invoke(main,'screen_status');assert.equal(video.phase,'idle');assert.ok(video.path&&video.copied);files.push(video.path);
  const decoded=spawnSync(encoder,['-hide_banner','-loglevel','error','-i',video.path,'-vf','scale=1:1','-pix_fmt','rgb24','-f','rawvideo','-'],{windowsHide:true,maxBuffer:4*1024*1024});
  assert.equal(decoded.status,0,decoded.stderr.toString());
  let blue=0,green=0,red=0;for(let i=0;i<decoded.stdout.length;i+=3){const[r,g,b]=decoded.stdout.subarray(i,i+3);if(b>g*1.3&&b>r*1.3)blue++;if(g>b*1.3&&g>r*1.3)green++;if(r>g*1.3&&r>b*1.3)red++;}
  assert.ok(blue>5&&green>5,JSON.stringify({blue,green,red}));assert.equal(red,0,'paused screen is absent from the MP4');
  const streams=spawnSync(encoder,['-hide_banner','-i',video.path,'-f','null','-'],{windowsHide:true,encoding:'utf8'});
  assert.equal(streams.status,0,streams.stderr);assert.equal(streams.stderr.includes('Audio: aac'),audio!=='none');
  if(audio!=='none')assert.match(streams.stderr,/48000 Hz, stereo/);
  for(const label of ['screen-select-0','screen-ink','screen-tools','screen-hud'])assert.equal(await invoke(main,'plugin:window|is_visible',{label}),false);
  // Cancel both while recording and while paused. Neither adds a history entry.
  const beforeCancel=await invoke(main,'screen_recent');
  for(const pauseFirst of [false,true]) {
    await select('video');await wait(async()=>(await invoke(main,'screen_status')).phase==='recording','cancel recording');
    await sleep(350);
    if(pauseFirst){await invoke(main,'screen_pause');await wait(async()=>(await invoke(main,'screen_status')).phase==='paused','cancel paused');}
    await invoke(main,'screen_cancel');
    assert.deepEqual(await invoke(main,'screen_status'),{phase:'idle',seconds:0,path:'',error:'',copied:false});
    assert.deepEqual(await invoke(main,'screen_recent'),beforeCancel);
  }
  // Release-to-copy skips all editing hosts and preserves full native dimensions.
  await invoke(main,'screen_save_preferences',{preferences:{...preferences,audio:'none',image_auto_copy:true}});
  await select('image');
  await wait(async()=>{const state=await invoke(main,'screen_status');return state.phase==='idle'&&state.copied;},'instant PNG copy');
  assert.equal(await invoke(main,'screen_editor_context'),null);
  for(const label of ['screen-select-0','screen-ink','screen-tools','screen-hud'])assert.equal(await invoke(main,'plugin:window|is_visible',{label}),false);
  const image=(await invoke(main,'screen_recent'))[0];assert.equal(image.kind,'image');files.push(image.path);
  const bytes=fs.readFileSync(image.path);assert.equal(bytes.readUInt32BE(16),Math.round(320*scale));assert.equal(bytes.readUInt32BE(20),Math.round(200*scale));
  assert.equal(await overlay.evaluate(()=>performance.getEntriesByType('navigation').length),1);
  console.log(JSON.stringify({passed:true,audio,blueFrames:blue,greenFrames:green,pausedFrames:red,activeSeconds:video.seconds,autoCopy:true,pausedStop:true,cancelRecording:true,cancelPaused:true,atomicShortcutSwap:true}));
} catch(error) {
  console.error('Native capture status:',await invoke(main,'screen_status'));
  for(const page of context.pages()) console.error(await page.evaluate(()=>({label:window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label,notice:document.querySelector('.screen-selection-help,[role="alert"]')?.textContent,selection:document.querySelector('.screen-selection-rect')?.getAttribute('style')})).catch(()=>null));
  throw error;
} finally {
  if(owned){try{await invoke(main,'screen_cancel');await invoke(main,'screen_cancel_selection');await invoke(main,'save_all_shortcuts',{voice:settings.hotkey,video:preferences.hotkey,image:preferences.image_hotkey});await invoke(main,'screen_save_preferences',{preferences});}catch(e){console.error('Preference restore failed',String(e));}}
  if(owned)restoreCaptureHistory(historyBefore,files);
  fs.writeFileSync(path.join(output,'state.json'),JSON.stringify({historyBefore,files},null,2));
  try{await main.reload();}catch{}await browser.close();
}
