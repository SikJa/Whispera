import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false,hmr:false}});await server?.listen();
const browser=await launchSilentBrowser();
try{
  const page=await browser.newPage({viewport:{width:640,height:480}});page.setDefaultTimeout(5000);
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{const original=window.__TAURI_INTERNALS__.invoke;let fail=true;window.__TAURI_INTERNALS__.invoke=async(command,args)=>{if(command==='screen_start'&&fail){fail=false;throw Error('No se pudo iniciar la captura de prueba');}return original(command,args);};});
  await page.goto(`${process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`}/?view=screen-select`,{timeout:90000});
  await page.locator('.screen-selection').waitFor();
  await page.mouse.move(100,100);await page.mouse.down();await page.mouse.move(500,350);await page.mouse.up();
  await page.getByText(/No se pudo iniciar la captura de prueba/).waitFor();
  assert.equal(await page.locator('.screen-selection').count(),1,'Failure must keep a usable selector');
  await page.mouse.move(80,90);await page.mouse.down();await page.mouse.move(480,290);await page.mouse.up();
  await page.getByTestId('recording-frame').waitFor();
  assert.deepEqual(await page.evaluate(()=>window.calls.find(c=>c.command==='screen_start').args.rect),{x:80,y:90,width:400,height:200});
  console.log('PASS: failed native capture start keeps error visible and permits another selection without Escape/reopen. IPC mocked.');
}finally{await browser.close();await server?.close();}
