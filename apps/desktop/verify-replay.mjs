import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server.listen();const browser=await launchSilentBrowser();
try{
const page=await browser.newPage({viewport:{width:1000,height:720}});const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.addInitScript(installNativeMock,{});
await page.addInitScript(()=>{window.__TAURI_INTERNALS__.convertFileSrc=()=> 'data:video/mp4;base64,';window.replayPrefs={enabled:false,seconds:60,audio:'none',folder:'',hotkey:''};const invoke=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{window.calls.push({command,args});if(command==='replay_preferences')return window.replayPrefs;if(command==='replay_status')return{phase:'off',availableSeconds:0,error:''};if(command==='plugin:dialog|open')return 'C:\\QA';if(command==='replay_create_folder')return 'C:\\QA\\Whispera-Repeticiones';if(command==='replay_save_preferences'){if(window.failReplay)throw Error('No se pudo guardar');window.replayPrefs=args.value;return;}if(command==='video_trim_context')return{path:'C:\\QA\\clip.mp4',name:'clip.mp4',duration:60,hasAudio:true};if(command==='video_trim_save')return new Promise(resolve=>window.finishTrim=()=>resolve({path:'C:\\QA\\trim.mp4',transcript:true,warning:''}));if(command==='video_trim_close')return;return invoke(command,args);};});
await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/`,{timeout:90000});await page.getByRole('button',{name:'Capturas y video',exact:true}).click();await page.locator('.replay-settings summary').click();await page.waitForTimeout(300);await page.locator('.replay-settings').evaluate(e=>e.open=true);
const enable=page.getByRole('switch',{name:'Activar grabación hacia atrás'});await enable.waitFor();assert.equal(await enable.isDisabled(),true);
await page.locator('#replay-folder').click();await page.locator('#replay-folder').filter({hasText:'Cambiar ubicación'}).waitFor();assert.equal(await enable.isDisabled(),true);
await page.locator('#replay-hotkey').click();await page.waitForTimeout(120);await page.keyboard.press('Control+Alt+F11');await page.waitForTimeout(200);assert.equal(await enable.isDisabled(),false);await enable.click();await page.waitForTimeout(150);assert.equal(await page.evaluate(()=>window.replayPrefs.enabled),true);
await page.locator('#replay-audio').selectOption('system');await page.waitForTimeout(120);assert.equal(await page.evaluate(()=>window.replayPrefs.audio),'system');
await page.locator('#replay-seconds').fill('10');await page.locator('#replay-seconds').blur();await page.waitForTimeout(120);assert.equal(await page.evaluate(()=>window.replayPrefs.seconds),10);
await page.evaluate(()=>window.emitNative('replay-status',{phase:'buffering',availableSeconds:5,encoder:'test',error:''}));
await page.getByRole('button',{name:'Elegir tramo',exact:true}).click();
await page.getByText('Preparación solicitada. El editor se abre automáticamente.',{exact:true}).waitFor();
assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='replay_save')));
await page.evaluate(()=>window.failReplay=true);await enable.click();await page.getByText(/No se pudo guardar/).waitFor();assert.equal(await enable.getAttribute('aria-checked'),'true');
await page.setViewportSize({width:520,height:440});await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/?view=video-trim`,{timeout:90000});await page.getByRole('button',{name:'Guardar',exact:false}).waitFor();
assert.equal(await page.locator('.trim-editor').evaluate(e=>e.scrollHeight<=e.clientHeight+1&&e.scrollWidth<=e.clientWidth+1),true,'Trim editor fits minimum native window');
await page.getByRole('spinbutton',{name:'Inicio del tramo'}).fill('12');await page.getByRole('spinbutton',{name:'Fin del tramo'}).fill('24');
await page.keyboard.press('Control+c');assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='video_trim_save').length),0,'Numeric text keeps standard copy behavior');
await page.getByRole('slider',{name:'Seleccionar final',exact:true}).focus();await page.keyboard.press('Control+c');await page.waitForTimeout(100);
assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='video_trim_save').length),1,'Ctrl+C must save after interacting with a range slider');
await page.getByText('Preparando video y transcripción…',{exact:true}).waitFor();
assert.equal(await page.getByRole('button',{name:'Cerrar editor'}).isDisabled(),true);await page.keyboard.press('Control+c');const calls=await page.evaluate(()=>window.calls.filter(c=>c.command==='video_trim_save'));assert.equal(calls.length,1);assert.deepEqual(calls[0].args,{start:12,end:24});await page.evaluate(()=>window.finishTrim());await page.getByText('Tramo guardado y copiado. Pegalo con Ctrl+V.').waitFor();await page.keyboard.press('Escape');assert.equal(await page.evaluate(()=>window.calls.some(c=>c.command==='video_trim_close')),true);assert.deepEqual(errors,[]);
console.log('PASS: replay folder and shortcut required, autosave, audio, duration and failed-save rollback; minimum-size trimmer, selected interval Ctrl+C, duplicate-save guard, busy close and Escape. Native IPC mocked.');
}finally{await browser.close();await server.close();}
