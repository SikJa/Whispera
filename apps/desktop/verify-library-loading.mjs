import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});await server?.listen();
const browser=await launchSilentBrowser();
try{
 const page=await browser.newPage();await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{window.failPrefs=true;const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args)=>{if(command==='library_preferences'){if(window.failPrefs)throw Error('Lectura de prueba fallida');return{captureGlobal:false,incognito:true,historyLimit:400,autoDeleteHours:0,clearUnpinnedOnRestart:false,transcribeVideo:false,videoTranscriptAttachment:false};}return original(command,args);};});
 await page.goto(process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`,{timeout:90000});
 await page.getByRole('button',{name:'Portapapeles',exact:true}).click();await page.getByRole('tab',{name:'Configuración',exact:true}).click();
 await page.getByRole('alert').filter({hasText:'Lectura de prueba fallida'}).waitFor();
 assert.equal(await page.locator('#library-limit').isDisabled(),true,'Failed read must never enable defaults that overwrite saved settings');
 assert.equal(await page.evaluate(()=>window.calls.some(c=>c.command==='library_action'&&c.args.action==='settings')),false);
 await page.evaluate(()=>window.failPrefs=false);await page.getByRole('button',{name:'Reintentar lectura',exact:true}).click();
 await page.waitForFunction(()=>!document.querySelector('#library-limit').disabled&&!document.querySelector('#library-limit').closest('fieldset').disabled);
 assert.equal(await page.locator('#library-limit').inputValue(),'400');
 await page.locator('#library-limit').fill('410');await page.waitForFunction(()=>window.libraryPreferences.historyLimit===410);
 const saved=await page.evaluate(()=>window.libraryPreferences);
 assert.equal(saved.incognito,true);assert.equal(saved.captureGlobal,false);assert.equal(saved.autoDeleteHours,0);assert.equal(saved.transcribeVideo,false);assert.equal(saved.videoTranscriptAttachment,false);
 console.log('PASS: library read failure blocks edits; retry restores saved preferences before later edits. Native IPC mocked.');
}finally{await browser.close();await server?.close();}
