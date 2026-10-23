import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false,hmr:false}});await server?.listen();
const browser=await launchSilentBrowser();
try{
  const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
  for(const broken of ['screen_appearance','screen_selection_image','screen_overlay_ready']) {
    const failed=await browser.newPage();
    await failed.addInitScript(installNativeMock,{kind:'image'});
    await failed.addInitScript(broken=>{
      const original=window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
        if(command===broken)throw Error('Simulated selector initialization failure');
        return original(command,args);
      };
    },broken);
    await failed.goto(`${base}/overlay.html?view=screen-select`);
    await failed.waitForFunction(()=>window.calls.some(c=>c.command==='screen_cancel_selection'));
    assert.equal(await failed.locator('.screen-selection').count(),0,'failed selector must release its native session');
    await failed.close();
  }
  const page=await browser.newPage({viewport:{width:640,height:480}});page.setDefaultTimeout(5000);
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{const original=window.__TAURI_INTERNALS__.invoke;let fail=true;window.__TAURI_INTERNALS__.invoke=async(command,args)=>{if(command==='screen_start'&&fail){fail=false;throw Error('No se pudo iniciar la captura de prueba');}return original(command,args);};});
  await page.goto(`${process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`}/?view=screen-select`,{timeout:90000});
  await page.locator('.screen-selection').waitFor();
  await page.mouse.move(100,300);await page.mouse.down();await page.mouse.move(500,350);await page.mouse.up();
  await page.getByText(/No se pudo iniciar la captura de prueba/).waitFor();
  assert.equal(await page.locator('.screen-selection').count(),1,'Failure must keep a usable selector');
  await page.mouse.move(80,280);await page.mouse.down();await page.mouse.move(480,470);await page.mouse.up();
  await page.getByTestId('recording-frame').waitFor();
  assert.deepEqual(await page.evaluate(()=>window.calls.find(c=>c.command==='screen_start').args.rect),{x:80,y:280,width:400,height:190});
  for(const endpoint of [{x:950,y:700},{x:-300,y:-150}]) {
    const edge=await browser.newPage({viewport:{width:640,height:480}});
    await edge.addInitScript(installNativeMock,{});
    await edge.goto(`${process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`}/?view=screen-select`);
    await edge.locator('.screen-selection').waitFor();
    await edge.mouse.move(200,280);await edge.mouse.down();
    await edge.mouse.move(endpoint.x,endpoint.y);await edge.mouse.up();
    await edge.getByTestId('recording-frame').waitFor();
    assert.deepEqual(await edge.evaluate(()=>window.calls.find(c=>c.command==='screen_start').args.rect),
      endpoint.x>0?{x:200,y:280,width:440,height:200}:{x:0,y:0,width:200,height:280},'cross-monitor selection stays inside its source viewport');
    await edge.close();
  }
  for(const interruptedBy of ['lostpointercapture','pointercancel']) {
    const interrupted=await browser.newPage({viewport:{width:640,height:480}});
    await interrupted.addInitScript(installNativeMock,{});
    await interrupted.goto(`${process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`}/?view=screen-select`);
    const selection=interrupted.locator('.screen-selection');await selection.waitFor();
    await selection.evaluate(element=>element.addEventListener('pointerdown',e=>window.dragPointerId=e.pointerId,{once:true}));
    await interrupted.mouse.move(100,300);await interrupted.mouse.down();await interrupted.mouse.move(300,250);
    if(interruptedBy==='lostpointercapture') {
      await selection.evaluate(element=>element.releasePointerCapture(window.dragPointerId));
      await interrupted.mouse.move(301,251);
    } else await selection.dispatchEvent('pointercancel',{pointerId:1});
    await interrupted.mouse.up();
    assert.equal(await interrupted.evaluate(()=>window.calls.filter(c=>c.command==='screen_start').length),0,'interrupted selection must not start a stale crop');
    assert.equal(await interrupted.locator('.screen-selection-rect').count(),0);
    await interrupted.mouse.move(80,280);await interrupted.mouse.down();await interrupted.mouse.move(480,470);await interrupted.mouse.up();
    await interrupted.getByTestId('recording-frame').waitFor();
    await interrupted.close();
  }
  console.log('PASS: capture failure/retry, cross-monitor bounds in both directions and cancellation/lost-pointer recovery. IPC mocked.');
}finally{await browser.close();await server?.close();}
