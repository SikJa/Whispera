import assert from 'node:assert/strict';
import {mkdirSync} from 'node:fs';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;const browser=await launchSilentBrowser();
try{
const page=await browser.newPage({viewport:{width:1000,height:720}});const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.addInitScript(installNativeMock,{});
await page.addInitScript(()=>{window.__TAURI_INTERNALS__.convertFileSrc=()=> 'data:video/mp4;base64,';window.replayPrefs={enabled:false,seconds:60,audio:'none',folder:'',hotkey:''};const invoke=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{window.calls.push({command,args});if(command==='replay_preferences')return window.replayPrefs;if(command==='replay_status')return{phase:'off',availableSeconds:0,error:''};if(command==='plugin:dialog|open')return window.cancelReplayFolder?null:'C:\\QA';if(command==='replay_create_folder')return 'C:\\QA\\Whispera-Repeticiones';if(command==='replay_save_preferences'){if(window.failReplay)throw Error('No se pudo guardar');if(window.delayReplaySave)await new Promise(resolve=>window.finishReplaySave=resolve);window.replayPrefs=args.value;return;}if(command==='video_trim_context')return{path:'C:\\QA\\clip.mp4',name:'clip.mp4',duration:60,hasAudio:true};if(command==='video_trim_save')return new Promise(resolve=>window.finishTrim=()=>resolve({path:'C:\\QA\\trim.mp4',transcript:true,warning:''}));if(command==='video_trim_close')return;return invoke(command,args);};});
await page.goto(base+'/',{timeout:90000});await page.getByRole('button',{name:'Capturas y video',exact:true}).click();
const details=page.locator('.replay-settings'),summary=details.locator('summary');
assert.equal(await details.evaluate(e=>e.open),false);
await summary.getByText('Configurar',{exact:true}).waitFor();
if(process.env.WHISPERA_REPLAY_SCREENSHOTS){mkdirSync(process.env.WHISPERA_REPLAY_SCREENSHOTS,{recursive:true});await details.screenshot({path:process.env.WHISPERA_REPLAY_SCREENSHOTS+'/replay-cerrado.png'});}
await summary.focus();await page.keyboard.press('Enter');await page.waitForFunction(()=>document.querySelector('.replay-settings').open);
await summary.getByText('Ocultar',{exact:true}).waitFor();
const enable=page.locator('#replay-enabled');await enable.waitFor();await page.waitForFunction(()=>!document.querySelector('#replay-enabled').disabled);
assert.equal(await enable.textContent(),'Configurar y activar');
if(process.env.WHISPERA_REPLAY_SCREENSHOTS){await details.screenshot({path:process.env.WHISPERA_REPLAY_SCREENSHOTS+'/replay-configurar.png'});}
await page.evaluate(()=>window.cancelReplayFolder=true);await enable.click();await page.waitForFunction(()=>!document.querySelector('#replay-enabled').disabled);
assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),false);assert.equal(await page.evaluate(()=>window.replayPrefs.folder),'');
await page.evaluate(()=>window.cancelReplayFolder=false);await enable.click();await page.locator('#replay-folder').filter({hasText:'Cambiar ubicación'}).waitFor();
assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),false,'Folder creation never starts capture');
await enable.click();await page.waitForFunction(()=>document.activeElement?.id==='replay-hotkey');await page.waitForTimeout(120);await page.keyboard.press('Control+Alt+F11');
await page.getByRole('button',{name:'Activar grabación',exact:true}).waitFor();
if(process.env.WHISPERA_REPLAY_SCREENSHOTS){await details.screenshot({path:process.env.WHISPERA_REPLAY_SCREENSHOTS+'/replay-listo.png'});}
assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),false,'Choosing the shortcut still requires explicit activation');
await enable.click();await page.getByRole('button',{name:'Desactivar grabación',exact:true}).waitFor();assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),true);
for(const width of [620,900,1000]){await page.setViewportSize({width,height:720});assert.equal(await details.evaluate(e=>e.scrollWidth<=e.clientWidth+1),true,'Replay card must not overflow at '+width+'px');}
// Repeat toggling and reopening: state must survive without recording anything.
for(let repeat=0;repeat<3;repeat++){
 await enable.click();await page.getByRole('button',{name:'Activar grabación',exact:true}).waitFor();assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),false);
 await enable.click();await page.getByRole('button',{name:'Desactivar grabación',exact:true}).waitFor();assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),true);
 await page.getByRole('button',{name:'Transcripción',exact:true}).click();await page.getByRole('button',{name:'Capturas y video',exact:true}).click();await summary.click();await page.getByRole('button',{name:'Desactivar grabación',exact:true}).waitFor();
}
await page.evaluate(()=>{window.delayReplaySave=true;window.beforeReplayCalls=window.calls.filter(c=>c.command==='replay_save_preferences').length;document.querySelector('#replay-enabled').click();document.querySelector('#replay-enabled').click();});
await page.getByRole('button',{name:'Guardando…',exact:true}).waitFor();assert.equal(await enable.isDisabled(),true);
assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='replay_save_preferences').length-window.beforeReplayCalls),1,'Rapid clicks must not duplicate preference saves');
await page.evaluate(()=>{window.delayReplaySave=false;window.finishReplaySave();});await page.getByRole('button',{name:'Activar grabación',exact:true}).waitFor();await enable.click();await page.getByRole('button',{name:'Desactivar grabación',exact:true}).waitFor();
await page.locator('#replay-audio').selectOption('system');await page.waitForTimeout(120);assert.equal(await page.evaluate(()=>window.replayPrefs.audio),'system');
await page.locator('#replay-seconds').fill('10');await page.locator('#replay-seconds').blur();await page.waitForTimeout(120);assert.equal(await page.evaluate(()=>window.replayPrefs.seconds),10);
await page.evaluate(()=>window.emitNative('replay-status',{phase:'buffering',availableSeconds:5,encoder:'test',error:''}));
await page.getByRole('button',{name:'Elegir tramo',exact:true}).click();
await page.getByText('Preparación solicitada. El editor se abre automáticamente.',{exact:true}).waitFor();
assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='replay_save')));
await page.evaluate(()=>window.failReplay=true);await enable.click();await page.getByText(/No se pudo guardar/).waitFor();assert.equal(await enable.textContent(),'Desactivar grabación');assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),true);
await page.setViewportSize({width:520,height:440});await page.goto(base+'/?view=video-trim',{timeout:90000});await page.getByRole('button',{name:'Guardar',exact:false}).waitFor();
assert.equal(await page.locator('.trim-editor').evaluate(e=>e.scrollHeight<=e.clientHeight+1&&e.scrollWidth<=e.clientWidth+1),true,'Trim editor fits minimum native window');
await page.getByRole('spinbutton',{name:'Inicio del tramo'}).fill('12');await page.getByRole('spinbutton',{name:'Fin del tramo'}).fill('24');
await page.keyboard.press('Control+c');assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='video_trim_save').length),0,'Numeric text keeps standard copy behavior');
await page.getByRole('slider',{name:'Seleccionar final',exact:true}).focus();await page.keyboard.press('Control+c');await page.waitForTimeout(100);
assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='video_trim_save').length),1,'Ctrl+C must save after interacting with a range slider');
await page.getByText('Preparando video y transcripción…',{exact:true}).waitFor();
assert.equal(await page.getByRole('button',{name:'Cerrar editor'}).isDisabled(),true);await page.keyboard.press('Control+c');const calls=await page.evaluate(()=>window.calls.filter(c=>c.command==='video_trim_save'));assert.equal(calls.length,1);assert.deepEqual(calls[0].args,{start:12,end:24});await page.evaluate(()=>window.finishTrim());await page.getByText('Tramo guardado y copiado. Pegalo con Ctrl+V.').waitFor();await page.keyboard.press('Escape');assert.equal(await page.evaluate(()=>window.calls.some(c=>c.command==='video_trim_close')),true);assert.deepEqual(errors,[]);
console.log('PASS: visible keyboard disclosure, guided setup, folder cancellation, explicit activation, three toggle/reopen cycles, rapid-click guard, autosave, audio, duration and failed-save rollback; minimum-size trimmer, selected interval Ctrl+C, duplicate-save guard, busy close and Escape. Native IPC mocked.');
}finally{await browser.close();await server?.close();}
