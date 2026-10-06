import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const bmp=Buffer.alloc(70);bmp.write('BM');bmp.writeUInt32LE(70,2);bmp.writeUInt32LE(54,10);
bmp.writeUInt32LE(40,14);bmp.writeInt32LE(2,18);bmp.writeInt32LE(-2,22);bmp.writeUInt16LE(1,26);bmp.writeUInt16LE(32,28);
Buffer.from([0,0,255,0,0,255,0,0,255,0,0,0,255,255,255,0]).copy(bmp,54);
const browser=await launchSilentBrowser(),base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';
try {
  const image=await browser.newPage({viewport:{width:640,height:480}}),errors=[];
  image.on('pageerror',e=>errors.push(e.message));
  await image.addInitScript(installNativeMock,{kind:'image',selectionPhase:'selecting',frozenImage:[...bmp]});
  await image.goto(base+'/overlay.html?view=screen-freeze');
  const frozen=image.getByLabel('Pantalla congelada para captura');await frozen.waitFor();
  await image.waitForFunction(()=>window.calls.some(c=>c.command==='screen_frozen_ready'));
  assert.deepEqual(await frozen.boundingBox(),{x:0,y:0,width:640,height:480});
  assert.equal(await frozen.evaluate(e=>getComputedStyle(e).pointerEvents),'auto');
  await image.evaluate(()=>{
    const button=document.createElement('button');button.id='behind';button.textContent='App detrás';
    Object.assign(button.style,{position:'fixed',left:'0',top:'0',width:'640px',height:'480px'});
    button.onclick=()=>window.behindClicks=(window.behindClicks??0)+1;
    document.body.insertBefore(button,document.getElementById('root'));
  });
  await image.mouse.click(100,100);
  assert.equal(await image.evaluate(()=>window.behindClicks??0),0,'the frozen surface consumes clicks');
  const url=await frozen.evaluate(e=>e.style.backgroundImage);
  await image.evaluate(()=>{
    window.videoStatus.phase='editing';
    window.emitNative('screen-editor-reset',{...window.editorContext,rect:{x:40,y:30,width:300,height:200}});
  });
  assert.equal(await frozen.evaluate(e=>e.style.backgroundImage),url,'editing and resizing preserve the same frozen desktop');
  assert.equal(await image.evaluate(()=>window.calls.filter(c=>c.command==='screen_selection_image').length),1);
  assert.equal(await image.evaluate(()=>window.calls.some(c=>c.command==='ui_heartbeat')),false,'static freeze views have no periodic heartbeat');
  await image.evaluate(()=>window.emitNative('screen-hide'));
  await frozen.waitFor({state:'detached'});
  await image.mouse.click(100,100);
  assert.equal(await image.evaluate(()=>window.behindClicks),1,'closing the surface releases clicks');

  await image.evaluate(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=(command,args)=>{
      if(command==='screen_selection_image'&&window.delayFrozen){window.delayFrozen=false;return new Promise((resolve,reject)=>{window.rejectOldFrozen=reject;});}
      return original(command,args);
    };
    window.delayFrozen=true;window.emitNative('screen-frozen-reset',2);
  });
  await image.waitForFunction(()=>!!window.rejectOldFrozen);
  await image.evaluate(()=>window.emitNative('screen-frozen-reset',3));
  await image.waitForFunction(()=>window.calls.some(c=>c.command==='screen_frozen_ready'&&c.args.generation===3));
  await image.evaluate(()=>window.rejectOldFrozen(Error('Respuesta atrasada')));
  await image.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  assert.equal(await image.evaluate(()=>window.calls.some(c=>c.command==='screen_cancel_selection')),false,'an old load failure cannot cancel the newer capture');
  assert.deepEqual(errors,[]);

  const video=await browser.newPage();await video.addInitScript(installNativeMock,{kind:'video',selectionPhase:'selecting',frozenImage:[...bmp]});
  await video.goto(base+'/overlay.html?view=screen-freeze');
  await video.waitForFunction(()=>window.calls.some(c=>c.command==='screen_frozen_state'));
  assert.equal(await video.getByLabel('Pantalla congelada para captura').count(),0);
  assert.equal(await video.evaluate(()=>window.calls.some(c=>['screen_selection_image','screen_frozen_ready'].includes(c.command))),false,'video never loads or shows a frozen desktop');
  console.log('PASS: full frozen image through editing, click blocking/release, one bitmap load, stale failure isolation, no timer and live-video exclusion. IPC mocked.');
} finally {await browser.close();}
