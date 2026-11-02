import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});
await server?.listen();
const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
const browser=await launchSilentBrowser();
try{
 const page=await browser.newPage({viewport:{width:1100,height:850}}),errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{
  const original=window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
   if(command==='screen_pending')return [{id:'test-pending',error:'Encoder agotado. Microfono conservado en Audios pendientes.'}];
   return original(command,args);
  };
 });
 for(let pass=0;pass<2;pass++){
  await page.goto(base+'/');
  await page.getByRole('button',{name:'Capturas y video',exact:true}).click();
  const pending=page.getByRole('region',{name:'Videos pendientes de recuperación'});
  await pending.waitFor();
  await pending.getByRole('button',{name:'Ver originales',exact:true}).click();
  assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='screen_pending_reveal'&&c.args.id==='test-pending')));
 }
 assert.deepEqual(errors,[]);
 console.log('PASS: failed video remains discoverable after reload, with original-file action. Silent headless; native IPC mocked.');
}finally{await browser.close();await server?.close();}
