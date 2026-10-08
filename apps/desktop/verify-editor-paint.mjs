import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:640,height:480}});
  await page.addInitScript(installNativeMock,{kind:'image',width:400,height:300});
  await page.addInitScript(()=>{
    window.pendingCrops=[];
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(['screen_editor_image','screen_editor_background'].includes(command)&&window.holdCrop) {
        const canvas=document.createElement('canvas');
        canvas.width=640;canvas.height=480;
        const ctx=canvas.getContext('2d');ctx.fillStyle=window.editorContext.width===320?'#2244aa':'#44aa22';ctx.fillRect(0,0,canvas.width,canvas.height);
        const bytes=Uint8Array.from(atob(canvas.toDataURL('image/png').split(',')[1]),v=>v.charCodeAt(0)).buffer;
        return new Promise(resolve=>window.pendingCrops.push(()=>resolve(bytes)));
      }
      return original(command,args);
    };
  });
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-ink');
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready')&&!window.feedback.busy);
  await page.evaluate(()=>{
    window.holdCrop=true;
    window.editorContext={...window.editorContext,width:300,height:220,rect:{x:20,y:30,width:300,height:220}};
    window.emitNative('screen-editor-reset',window.editorContext);
  });
  await page.waitForFunction(()=>window.pendingCrops.length===1&&window.feedback.busy);
  await page.evaluate(()=>{
    window.editorContext={...window.editorContext,width:320,height:240,rect:{x:40,y:50,width:320,height:240}};
    window.emitNative('screen-editor-reset',window.editorContext);
  });
  await page.waitForFunction(()=>window.pendingCrops.length===2);
  const frames=await page.evaluate(async()=>{
    const pixels=[];
    for(let i=0;i<8;i++) {
      await new Promise(requestAnimationFrame);
      pixels.push([...document.querySelector('canvas').getContext('2d').getImageData(10,10,1,1).data]);
    }
    return pixels;
  });
  assert.ok(frames.every(pixel=>pixel[3]===255),'the canvas must never go transparent while successive crops are loading');
  await page.evaluate(()=>window.pendingCrops[1]());
  await page.waitForFunction(()=>!window.feedback.busy);
  assert.deepEqual(await page.locator('canvas').evaluate(e=>[...e.getContext('2d').getImageData(10,10,1,1).data]),[34,68,170,255]);
  await page.evaluate(()=>window.pendingCrops[0]());
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  assert.deepEqual(await page.locator('canvas').evaluate(e=>[...e.getContext('2d').getImageData(10,10,1,1).data]),[34,68,170,255],'a late older crop cannot replace the latest image');
  await page.evaluate(()=>{
    window.editorContext={...window.editorContext,width:350,height:260,rect:{x:60,y:70,width:350,height:260}};
    window.emitNative('screen-editor-reset',window.editorContext);
  });
  await page.waitForFunction(()=>document.querySelector('canvas').width===350);
  assert.equal(await page.evaluate(()=>window.pendingCrops.length),2,'subsequent crops use the same frozen bitmap without fetching new pixels');
  assert.deepEqual(await page.locator('canvas').evaluate(e=>[...e.getContext('2d').getImageData(340,250,1,1).data]),[34,68,170,255]);
  await page.keyboard.press('Control+c');
  await page.waitForFunction(()=>window.exports.length===1);
  console.log('PASS: opaque crop preview throughout successive resize frames, binary PNG loading, stale crop rejection and export recovery. IPC mocked.');
} finally {await browser.close();}
