import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
 const page=await browser.newPage({viewport:{width:640,height:480}});
 await page.addInitScript(installNativeMock,{kind:'image',width:400,height:300});
 await page.addInitScript(()=>{
  const original=window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
   if(command==='screen_image_export'&&window.holdExport){window.calls.push({command,args});window.exports.push(args);return new Promise(resolve=>window.finishExport=resolve);}
   return original(command,args);
  };
 });
 await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-ink');
 await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready')&&!window.feedback.busy);
 await page.evaluate(()=>{window.failExport=true;window.toBlobCalls=0;const original=HTMLCanvasElement.prototype.toBlob;HTMLCanvasElement.prototype.toBlob=function(...args){window.toBlobCalls++;return original.apply(this,args);};});
 await page.keyboard.press('Control+c');await page.waitForFunction(()=>window.feedback.error?.includes('Portapapeles')&&!window.feedback.busy);
 assert.equal(await page.evaluate(()=>window.toBlobCalls),0,'an unmarked capture reuses the native crop without PNG/JSON conversion');
 await page.evaluate(()=>{window.failExport=false;window.holdExport=true;});
 await page.keyboard.press('Control+c');await page.waitForFunction(()=>!!window.finishExport&&window.feedback.busy);
 await page.keyboard.press('Control+c');assert.equal(await page.evaluate(()=>window.exports.length),1,'repeated Ctrl+C cannot queue another copy');
 await page.evaluate(()=>{window.editorContext={...window.editorContext,width:320,height:240,rect:{x:20,y:30,width:320,height:240}};window.emitNative('screen-editor-reset',window.editorContext);});
 await page.waitForFunction(()=>document.querySelector('canvas').width===320);await page.keyboard.press('Control+c');
 assert.equal(await page.evaluate(()=>window.exports.length),1,'a resize/readiness update cannot reset the in-flight export guard');
 assert.deepEqual(await page.evaluate(()=>window.exports[0].bytes),[]);
 await page.evaluate(()=>{window.holdExport=false;window.finishExport(false);});await page.waitForFunction(()=>!window.feedback.busy);
 await page.keyboard.press('Control+s');await page.waitForFunction(()=>window.exports.length===2);assert.equal(await page.evaluate(()=>window.exports[1].action),'save');
 await page.keyboard.press('a');await page.mouse.move(40,40);await page.mouse.down();await page.mouse.move(170,100);await page.mouse.up();await page.waitForFunction(()=>window.feedback.count===1);
 await page.keyboard.press('Control+c');await page.waitForFunction(()=>window.exports.length===3&&!window.feedback.busy);
 assert.ok(await page.evaluate(()=>window.exports[2].bytes.length>0),'annotated captures still export their composed pixels');
 assert.equal(await page.evaluate(()=>window.toBlobCalls),1,'only the composed image needs browser encoding');
 console.log('PASS: blank capture fast path, copy failure/retry, duplicate Ctrl+C, resize during copy, save cancellation and annotated export. Native IPC mocked.');
} finally {await browser.close();}
