import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {realpathSync} from 'node:fs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';

// Same top-down BI_RGB layout as the native frozen preview; no image compression.
const bmp=Buffer.alloc(70);
bmp.write('BM');bmp.writeUInt32LE(70,2);bmp.writeUInt32LE(54,10);
bmp.writeUInt32LE(40,14);bmp.writeInt32LE(2,18);bmp.writeInt32LE(-2,22);
bmp.writeUInt16LE(1,26);bmp.writeUInt16LE(32,28);
Buffer.from([0,0,255,0, 0,255,0,0, 255,0,0,0, 255,255,255,0]).copy(bmp,54);
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,hmr:false,fs:{allow:[process.cwd(),realpathSync('node_modules')]}}});
await server?.listen();
const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
const browser=await launchSilentBrowser();
try {
  const windows=await browser.newPage();
  windows.setDefaultTimeout(30000);
  await windows.addInitScript(()=>Object.defineProperty(navigator,'platform',{get:()=> 'Win32'}));
  await windows.addInitScript(installNativeMock,{kind:'image',selectionPhase:'selecting',frozenImage:[...bmp]});
  await windows.goto(base+'/?view=screen-select');
  await windows.waitForFunction(()=>window.calls.some(c=>c.command==='screen_overlay_ready'));
  assert.equal(await windows.evaluate(()=>window.calls.filter(c=>c.command==='screen_selection_image').length),0,'Windows selector must reuse the separate frozen layer, not transfer/decode a second desktop');
  assert.equal(await windows.locator('.screen-selection').evaluate(e=>e.style.backgroundImage),'');
  await windows.close();
  const page=await browser.newPage();
  await page.addInitScript(()=>Object.defineProperty(navigator,'platform',{get:()=> 'MacIntel'}));
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(installNativeMock,{kind:'image',selectionPhase:'idle',frozenImage:[...bmp]});
  await page.goto(base+'/?view=screen-select');
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_status'));
  assert.equal(await page.locator('.screen-selection').count(),0,'preloaded selector stays empty');
  assert.equal(await page.evaluate(()=>window.calls.some(c=>c.command==='screen_overlay_ready')),false);
  await page.evaluate(()=>{window.videoStatus.phase='selecting';window.emitNative('screen-reset','image');});
  await page.locator('.screen-selection[data-frozen=true]').waitFor();
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_overlay_ready'));
  assert.equal(await page.locator('.screen-selection').evaluate(e=>getComputedStyle(e).backgroundColor),'rgba(0, 0, 0, 0)');
  const pixels=await page.locator('.screen-selection').evaluate(async e=>{
    const image=new Image();image.src=e.style.backgroundImage.slice(5,-2);await image.decode();
    const canvas=document.createElement('canvas');canvas.width=2;canvas.height=2;
    const ctx=canvas.getContext('2d');ctx.drawImage(image,0,0);
    return [...ctx.getImageData(0,0,2,2).data];
  });
  assert.deepEqual(pixels,[255,0,0,255,0,255,0,255,0,0,255,255,255,255,255,255]);
  await page.evaluate(()=>window.emitNative('screen-hide'));
  await page.locator('.screen-selection').waitFor({state:'detached'});
  assert.deepEqual(errors,[]);
  console.log('PASS: hidden preloading, BMP decoding/orientation/color, transparent selection and cancellation. IPC mocked.');
} finally {await browser.close();await server?.close();}
