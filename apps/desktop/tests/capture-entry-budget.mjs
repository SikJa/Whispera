import assert from 'node:assert/strict';
import {preview} from 'vite';
import {launchSilentBrowser} from './silent-browser.mjs';
import {installNativeMock} from './native-mock.mjs';
const server=await preview({preview:{host:'127.0.0.1',port:5191,strictPort:true}});
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage(),responses=[];
  page.on('response',r=>{if(new URL(r.url()).pathname.endsWith('.js'))responses.push(r.body().then(b=>({path:new URL(r.url()).pathname,bytes:b.length})));});
  await page.addInitScript(installNativeMock,{kind:'image',selectionPhase:'selecting'});
  await page.goto('http://127.0.0.1:5191/overlay.html?view=screen-select');
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_overlay_ready'));
  const modules=await Promise.all(responses),bytes=modules.reduce((sum,m)=>sum+m.bytes,0);
  assert.ok(!modules.some(m=>m.path.includes('ScreenEditor')));
  assert.ok(bytes<300000,'built selector must not pull the full editor/settings bundle');
  console.log(JSON.stringify({scope:'Production selector JavaScript fetched through readiness; uncompressed, no native startup timing',modules,bytes}));
} finally {await browser.close();await new Promise(resolve=>server.httpServer.close(resolve));}
