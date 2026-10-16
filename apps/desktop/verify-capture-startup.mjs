import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';

function readyHarness(source,frameDelay,reply=()=>true) {
  const exports={},frames=new Map();let sequence=0,calls=0;
  vm.runInNewContext(ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS}}).outputText,{
    exports,setTimeout,clearTimeout,
    requestAnimationFrame(callback){const id=++sequence;if(frameDelay!==null)frames.set(id,setTimeout(()=>{frames.delete(id);callback();},frameDelay));return id;},
    cancelAnimationFrame(id){clearTimeout(frames.get(id));frames.delete(id);},
    require(){return {invoke:async()=>{calls++;return reply();}};},
  });
  return {show:exports.showWhenReady,calls:()=>calls,frames};
}
const source=readFileSync(new URL('./src/screen-ready.ts',import.meta.url),'utf8');
for(const delay of [null,250,5]) {
  const h=readyHarness(source,delay);const start=performance.now();
  await Promise.race([h.show('ready',{},()=>true),new Promise((_,reject)=>{const t=setTimeout(()=>reject(Error('Hidden frame deadlock')),1000);t.unref();})]);
  const elapsed=performance.now()-start;
  assert.equal(h.calls(),1);assert.equal(h.frames.size,0);assert.ok(elapsed<350);
  console.log(`Ready handshake, frame delay ${delay??'suspended'}: ${elapsed.toFixed(1)} ms (synthetic scheduler).`);
}
const cancelled=readyHarness(source,null);let alive=true;
const pending=cancelled.show('ready',{},()=>alive);alive=false;await pending;assert.equal(cancelled.calls(),0);
let attempts=0;const configuring=readyHarness(source,5,()=>++attempts>=3);
await configuring.show('ready',{},()=>true);assert.equal(attempts,3);
if(process.env.WHISPERA_READY_BASELINE) {
  for(const [name,text] of [['before',readFileSync(process.env.WHISPERA_READY_BASELINE,'utf8')],['after',source]]) {
    const h=readyHarness(text,250);const start=performance.now();await h.show('ready',{},()=>true);
    console.log(`Comparison ${name}: ${(performance.now()-start).toFixed(1)} ms; identical 250 ms frame scheduling, not end-to-end capture.`);
  }
}
const browser=await launchSilentBrowser();
const base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';
try {
  for(const kind of ['image','video']) for(const view of ['screen-tools','screen-hud']) {
    const page=await browser.newPage({viewport:{width:380,height:600}});
    await page.addInitScript(installNativeMock,{kind});
    await page.addInitScript(()=>{
      window.requestAnimationFrame=()=>0;window.cancelAnimationFrame=()=>{};
    });
    await page.goto(`${base}/overlay.html?view=${view}`);
    await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready'),null,{polling:20,timeout:10000});
    const expected=view==='screen-tools'?'Herramientas de captura':kind==='video'?'Controles de video':'Acciones de captura';
    assert.equal(await page.getByLabel(expected,{exact:true}).count(),1);
    assert.equal(await page.getByRole('alert').count(),0);
    await page.close();
  }
  console.log('PASS: hidden/suspended frames cannot strand image/video controls; cancellation, native configuration retries and timer cleanup. Native IPC mocked.');
} finally {await browser.close();}
