import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';

// Same top-down BI_RGB layout as the native frozen preview; no image compression.
const bmp=Buffer.alloc(70);
bmp.write('BM');bmp.writeUInt32LE(70,2);bmp.writeUInt32LE(54,10);
bmp.writeUInt32LE(40,14);bmp.writeInt32LE(2,18);bmp.writeInt32LE(-2,22);
bmp.writeUInt16LE(1,26);bmp.writeUInt16LE(32,28);
Buffer.from([0,0,255,0, 0,255,0,0, 255,0,0,0, 255,255,255,0]).copy(bmp,54);
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage();
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(installNativeMock,{kind:'image',selectionPhase:'idle',frozenImage:[...bmp]});
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/?view=screen-select');
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
} finally {await browser.close();}
