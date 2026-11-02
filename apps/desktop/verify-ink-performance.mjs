import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {PNG} from 'pngjs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:640,height:480}});
  await page.addInitScript(installNativeMock,{kind:'image',width:640,height:480});
  await page.addInitScript(()=>{
    window.lineCalls=0;
    const line=CanvasRenderingContext2D.prototype.lineTo;
    CanvasRenderingContext2D.prototype.lineTo=function(...args){window.lineCalls++;return line.apply(this,args);};
  });
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-ink');
  await page.waitForFunction(()=>window.feedback.busy===false);
  await page.keyboard.press('p');
  for(let stroke=0;stroke<12;stroke++) {
    await page.mouse.move(30,30+stroke*22);await page.mouse.down();
    await page.evaluate(stroke=>{
      const c=document.querySelector('canvas');
      for(let i=1;i<=600;i++)c.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,pointerId:1,clientX:30+i*.8,clientY:30+stroke*22+Math.sin(i*.04)*8}));
    },stroke);
    await page.mouse.up();
    await page.waitForFunction(n=>window.feedback.count===n,stroke+1);
  }
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await page.keyboard.press('a');await page.mouse.move(50,350);await page.mouse.down();
  await page.evaluate(()=>new Promise(requestAnimationFrame));
  await page.evaluate(()=>window.lineCalls=0);
  const elapsed=await page.evaluate(async()=>{
    const start=performance.now(),c=document.querySelector('canvas');
    for(let i=0;i<20;i++){
      c.dispatchEvent(new PointerEvent('pointermove',{bubbles:true,pointerId:1,clientX:100+i*5,clientY:350}));
      await new Promise(requestAnimationFrame);
    }
    return performance.now()-start;
  });
  const calls=await page.evaluate(()=>window.lineCalls);
  await page.mouse.up();await page.waitForFunction(()=>window.feedback.count===13);
  const hashes=[];
  for(const action of ['copy','undo','redo','clear']) {
    if(action!=='copy')await page.evaluate(action=>window.emitNative('screen-editor-action',{id:window.editorContext.id,action:{action}}),action);
    if(action==='clear'){await page.waitForFunction(()=>window.feedback.count===0);break;}
    const count=await page.evaluate(()=>window.exports.length);
    await page.keyboard.press('Control+c');await page.waitForFunction(n=>window.exports.length>n&&!window.feedback.busy,count);
    const bytes=await page.evaluate(()=>window.exports.at(-1).bytes);
    hashes.push(createHash('sha256').update(PNG.sync.read(Buffer.from(bytes)).data).digest('hex'));
  }
  assert.equal(hashes[0],hashes[2],'redo restores identical exported pixels');
  assert.notEqual(hashes[0],hashes[1],'undo removes the draft arrow');
  console.log(JSON.stringify({staticPoints:7212,frames:20,lineCalls:calls,elapsedMs:elapsed,pixelHashes:hashes}));
  assert.ok(calls<1000,`committed marks must not be replayed for every draft frame (${calls} line operations)`);
  console.log('PASS: dense ink reuses committed rendering; copy, undo, redo and clear preserved. IPC mocked.');
} finally {await browser.close();}
