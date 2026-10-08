import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();
const browser=await launchSilentBrowser();
try{
 const page=await browser.newPage();await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{const original=window.__TAURI_INTERNALS__.invoke;let first=true;window.videoPreferences={...window.videoPreferences,audio:'both',frame_color:'#2288cc'};window.__TAURI_INTERNALS__.invoke=async(command,args)=>{if(command==='screen_preferences'&&first){first=false;return new Promise((resolve,reject)=>{window.finishCaptureLoad=()=>resolve({...window.videoPreferences});window.failCaptureLoad=()=>reject(Error('Lectura de prueba fallida'));});}return original(command,args);};});
 await page.goto(process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`,{timeout:90000});
 await page.getByRole('button',{name:'Capturas y video',exact:true}).click();
 const toggle=page.getByRole('switch',{name:'Copiar al soltar la selección'});
 await page.waitForFunction(()=>!!window.finishCaptureLoad);
 assert.equal(await toggle.isDisabled(),true,'No editing defaults before persisted preferences arrive');
 await page.evaluate(()=>window.failCaptureLoad());
 await page.getByRole('button',{name:'Volver a cargar preferencias'}).click();
 await page.waitForFunction(()=>document.querySelector('#image-auto-copy')?.disabled===false);
 await toggle.click();await page.getByText('Guardado automáticamente',{exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.videoPreferences.audio),'both');assert.equal(await page.evaluate(()=>window.videoPreferences.frame_color),'#2288cc');
 assert.equal(await page.evaluate(()=>window.videoPreferences.image_auto_copy),true);
 console.log('PASS: delayed/failed capture preferences cannot overwrite saved audio or color; retry recovers and editing preserves unrelated values. IPC mocked.');
}finally{await browser.close();await server?.close();}
