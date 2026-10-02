// Run only against a local Whispera test process started with WebView2 CDP on 9223.
// Captures a synthetic pattern rendered in that app, never an unrelated app.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {DatabaseSync} from 'node:sqlite';
import {spawnSync} from 'node:child_process';
import {chromium} from 'playwright';
import {PNG} from 'pngjs';
import {restoreCaptureHistory} from './tests/native-history.mjs';
const root=path.resolve('../..'),out=path.join(root,'.local/native-check');fs.mkdirSync(out,{recursive:true});
const db=new DatabaseSync(path.join(process.env.APPDATA,'app.whispera.desktop','whispera.sqlite'),{readOnly:true});
const historyBefore=db.prepare('SELECT value FROM kv WHERE key=?').get('capture_history')?.value??null;db.close();
fs.writeFileSync(path.join(out,'state.json'),JSON.stringify({historyBefore,files:[]},null,2));
const browser=await chromium.connectOverCDP('http://127.0.0.1:9223'),context=browser.contexts()[0];
const invoke=(page,command,args={})=>page.evaluate(([command,args])=>window.__TAURI_INTERNALS__.invoke(command,args),[command,args]);
async function waitNative(check,description){for(let i=0;i<150;i++){if(await check())return;await new Promise(r=>setTimeout(r,100));}throw Error(`Native timeout: ${description}`);}
async function findWindow(label){for(let tries=0;tries<100;tries++){for(const page of context.pages()){try{if(await page.evaluate(()=>window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label)===label)return page;}catch{}}await new Promise(r=>setTimeout(r,100));}throw Error(`Window ${label} did not open`);}
const main=await findWindow('main');
const files=[];
const previousPreferences=await invoke(main,'screen_preferences');
assert.equal((await invoke(main,'screen_status')).phase,'idle','test requires idle screen capture');
assert.ok(!['recording','paused','processing'].includes((await invoke(main,'recording_state')).phase),'test requires idle dictation');
try{
  await invoke(main,'screen_save_preferences',{preferences:{...previousPreferences,audio:'none',image_auto_copy:false,frame_color:'#ffffff'}});
  await invoke(main,'open_settings');
  const pattern=await main.evaluate(()=>{
    const width=Math.min(640,innerWidth-160),height=Math.min(400,innerHeight-160),scale=devicePixelRatio;
    document.body.innerHTML='';document.body.style.background='#e6edf5';
    const canvas=document.createElement('canvas');canvas.id='native-test-pattern';canvas.width=Math.round(width*scale);canvas.height=Math.round(height*scale);
    Object.assign(canvas.style,{position:'fixed',left:'80px',top:'80px',width:`${width}px`,height:`${height}px`});document.body.append(canvas);
    const c=canvas.getContext('2d');c.scale(scale,scale);c.fillStyle='#e6edf5';c.fillRect(0,0,width,height);
    c.fillStyle='#216de0';c.fillRect(width/2,0,width/2,height/2);c.fillStyle='#15a67c';c.fillRect(width/2,height/2,width/2,height/2);
    c.fillStyle='#182131';c.font='600 23px Segoe UI';c.fillText('Whispera · Calidad 123',16,height-50);c.font='14px Segoe UI';c.fillText('Texto nítido / PNG / H.264',16,height-23);
    return{width,height,scale,png:canvas.toDataURL('image/png').split(',')[1]};
  });
  fs.writeFileSync(path.join(out,'reference.png'),Buffer.from(pattern.png,'base64'));
  await main.evaluate(()=>new Promise(requestAnimationFrame).then(()=>new Promise(requestAnimationFrame)));
  const position=await invoke(main,'plugin:window|inner_position',{label:'main'});
  async function select(kind){
    await invoke(main,kind==='image'?'screen_select_image':'screen_select');
    const overlay=await findWindow('screen-select-0');await overlay.locator('.screen-selection').waitFor({timeout:10000});
    await waitNative(async()=>await invoke(overlay,'plugin:window|is_visible',{label:'screen-select-0'})&&await invoke(overlay,'plugin:window|is_focused',{label:'screen-select-0'}),'selector ready');
    const origin=await invoke(overlay,'plugin:window|inner_position',{label:'screen-select-0'});
    const scale=await invoke(overlay,'plugin:window|scale_factor',{label:'screen-select-0'});
    const x=(position.x+80*pattern.scale-origin.x)/scale,y=(position.y+80*pattern.scale-origin.y)/scale;
    await overlay.mouse.move(x,y);await overlay.mouse.down();await overlay.mouse.move(x+pattern.width*pattern.scale/scale,y+pattern.height*pattern.scale/scale);await overlay.mouse.up();
    return overlay;
  }
  let overlay=await select('video');
  await waitNative(async()=> (await invoke(main,'screen_status')).phase==='recording','recording');
  await waitNative(async()=> (await invoke(main,'screen_editor_context'))?.kind==='video','video editor');
  const videoContext=await invoke(main,'screen_editor_context');
  const ink=await findWindow('screen-ink'),tools=await findWindow('screen-tools');
  await ink.locator(`.screen-ink[data-session="${videoContext.id}"]`).waitFor();
  await tools.locator(`.capture-toolbar[data-session="${videoContext.id}"]`).waitFor();
  // CDP input does not activate an unfocused native WebView2 host window.
  // Activate the real control in its DOM; native handling then focuses the ink host.
  await tools.getByLabel('Flecha',{exact:true}).waitFor();await tools.getByLabel('Flecha',{exact:true}).evaluate(el=>el.click());
  await ink.waitForFunction(()=>document.querySelector('.screen-ink')?.dataset.tool==='arrow');
  await ink.mouse.move(30,30);await ink.mouse.down();await ink.mouse.move(150,90);await ink.mouse.up();
  await waitNative(async()=> (await invoke(main,'screen_editor_feedback_get')).count===1,'arrow committed');
  const inkColor=(await invoke(main,'screen_editor_feedback_get')).color;
  await new Promise(r=>setTimeout(r,700));
  assert.equal(await overlay.evaluate(()=>performance.getEntriesByType('navigation').length),1);
  assert.ok(overlay.url().endsWith('view=screen-select'),'native overlay did not navigate when recording began');
  await invoke(main,'screen_stop');
  const video=await invoke(main,'screen_status');assert.ok(video.path&&video.copied,JSON.stringify(video));files.push(video.path);
  const encoder=path.resolve('node_modules/ffmpeg-static/ffmpeg.exe');
  const decoded=path.join(out,'video-last.png');
  const ff=spawnSync(encoder,['-hide_banner','-loglevel','error','-sseof','-0.25','-i',video.path,'-frames:v','1','-y',decoded],{windowsHide:true,encoding:'utf8'});assert.equal(ff.status,0,ff.stderr);
  const pixels=PNG.sync.read(fs.readFileSync(decoded));assert.equal(pixels.width,Math.ceil(pattern.width*pattern.scale/2)*2);assert.equal(pixels.height,Math.ceil(pattern.height*pattern.scale/2)*2);
  const reference=PNG.sync.read(fs.readFileSync(path.join(out,'reference.png')));
  let error=0,n=0;for(let y=0;y<Math.floor(pattern.height*pattern.scale);y++)for(let x=Math.floor(pattern.width*pattern.scale*.65);x<Math.floor(pattern.width*pattern.scale*.9);x++){for(let channel=0;channel<3;channel++){error+=Math.abs(pixels.data[(y*pixels.width+x)*4+channel]-reference.data[(y*reference.width+x)*4+channel]);n++;}}
  const mae=error/n;assert.ok(mae<6,`video quality or overlay exclusion failed: mean RGB error ${mae}`);
  const arrowOffset=(Math.round(30*pattern.scale)*pixels.width+Math.round(30*pattern.scale))*4;
  const expectedInk=[1,3,5].map(i=>parseInt(inkColor.slice(i,i+2),16));
  assert.ok(expectedInk.reduce((error,c,i)=>error+Math.abs(c-pixels.data[arrowOffset+i]),0)<65,'annotation retains its chosen color in the native MP4');
  overlay=await select('image');
  await waitNative(async()=> (await invoke(main,'screen_editor_context'))?.kind==='image','image editor');
  const imageContext=await invoke(main,'screen_editor_context');
  await ink.locator(`.screen-ink[data-session="${imageContext.id}"]`).waitFor();
  await tools.locator(`.capture-toolbar[data-session="${imageContext.id}"]`).waitFor();
  assert.equal(await ink.locator('.screen-ink').evaluate(e=>getComputedStyle(e,'::after').borderColor),'rgb(255, 255, 255)','image editor keeps its white frame above the opaque canvas');
  await ink.getByLabel('Editar captura').waitFor({timeout:15000});await tools.getByRole('button',{name:'Copiar',exact:true}).evaluate(el=>el.click());
  await waitNative(async()=> (await invoke(main,'screen_status')).phase==='idle','image copied');
  const recents=await invoke(main,'screen_recent');const image=recents.find(e=>e.kind==='image');assert.ok(image);files.push(image.path);
  const png=PNG.sync.read(fs.readFileSync(image.path));assert.equal(png.width,reference.width);assert.equal(png.height,reference.height);
  let imageError=0;for(let i=0;i<png.data.length;i+=4)for(let c=0;c<3;c++)imageError+=Math.abs(png.data[i+c]-reference.data[i+c]);
  const imageMae=imageError/(png.width*png.height*3);assert.ok(imageMae<1,`PNG differs from source: ${imageMae}`);
  assert.equal(await overlay.evaluate(()=>performance.getEntriesByType('navigation').length),1,'repeated capture reuses the native document');
  console.log(JSON.stringify({passed:true,png:[png.width,png.height],pngMeanRgbError:imageMae,video:[pixels.width,pixels.height],videoMeanRgbError:mae,videoCopied:video.copied,annotationsInVideo:true,overlayNavigationCount:1}));
}catch(error){
  for(const p of context.pages()){
    const label=await p.evaluate(()=>window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label).catch(()=>null);
    if(['screen-ink','screen-tools'].includes(label))console.log(label,await p.evaluate(()=>({tool:document.querySelector('.screen-ink')?.dataset.tool,error:document.querySelector('[role="alert"]')?.textContent,buttons:Array.from(document.querySelectorAll('[aria-pressed="true"]')).map(e=>e.getAttribute('aria-label'))})));
  }
  throw error;
}finally{
  try{await invoke(main,'screen_cancel_selection');await invoke(main,'screen_stop');}catch{}
  await invoke(main,'screen_save_preferences',{preferences:previousPreferences});
  const latest=await invoke(main,'screen_status');if(latest.path&&!files.includes(latest.path))files.push(latest.path);
  fs.writeFileSync(path.join(out,'state.json'),JSON.stringify({historyBefore,files},null,2));
  restoreCaptureHistory(historyBefore,files);
  try{await main.reload();}catch{}
  await browser.close();
}
