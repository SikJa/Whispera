import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();const browser=await launchSilentBrowser();
const prevented=(page,selector)=>page.evaluate(selector=>{const event=new MouseEvent('contextmenu',{bubbles:true,cancelable:true,button:2});document.querySelector(selector).dispatchEvent(event);return event.defaultPrevented;},selector);
try{
 const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
 // The folder with no microphone: the notice stays up, so it needs its own way out.
 const folder=await browser.newPage({viewport:{width:520,height:520},locale:'es-ES'});
 await folder.addInitScript(installNativeMock,{windowLabel:'recorder'});
 await folder.addInitScript(()=>{
  const original=window.__TAURI_INTERNALS__.invoke;
  window.voiceState={phase:'idle',seconds:0,error:'No se detectó micrófono',text:'',muted:false,progress:''};
  window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
   if(command==='recording_state'){window.calls.push({command,args});return structuredClone(window.voiceState);}
   if(command==='read_settings')return{color:'#9024DC',recorderScale:.85,pattern:'wave',dictationArtwork:'original',placement:'right'};
   // Rust clears the notice with the folder; the mock does the same.
   if(command==='recorder_close')window.voiceState.error='';
   return original(command,args);
  };
 });
 await folder.goto(base+'/?view=record',{timeout:90000});
 await folder.getByRole('alert').filter({hasText:'No se detectó micrófono'}).waitFor();
 const close=folder.getByRole('button',{name:'Cerrar grabadora',exact:true});
 const box=await close.boundingBox(),notice=await folder.getByRole('alert').boundingBox();
 assert.ok(box.x>=notice.x&&box.x+box.width<=notice.x+notice.width&&box.y>=notice.y&&box.y+box.height<=notice.y+notice.height,'the X sits inside the notice, so it is inside the interactive region');
 assert.ok(box.x+box.width<=520&&box.y+box.height<=520,'the X is inside the window');
 assert.equal(await prevented(folder,'.float-rig'),true,'Chrome menu is blocked over the folder');
 assert.equal(await prevented(folder,'.float-error-text'),true,'and over the notice');
 assert.equal(await folder.evaluate(()=>window.calls.filter(c=>c.command==='recorder_menu').length),2,'every right-click asks Rust for Whispera\'s menu');
 await close.click();
 assert.ok(await folder.evaluate(()=>window.calls.some(c=>c.command==='recorder_close')),'the X asks Rust to hide the folder');
 await folder.getByRole('alert').waitFor({state:'detached'});
 // Clicking the notice itself still opens the details, as before.
 await folder.evaluate(()=>{window.voiceState.error='Prueba de recuperación';});
 await folder.getByRole('alert').click();
 assert.ok(await folder.evaluate(()=>window.calls.some(c=>c.command==='open_recording_details')));

 // The popup is one window with two menus: everything for the tray icon, a short one for the folder.
 const menu=await browser.newPage({viewport:{width:344,height:480},locale:'es-ES'});
 await menu.addInitScript(installNativeMock,{windowLabel:'tray-menu'});
 await menu.addInitScript(()=>{
  const original=window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke=async(command,args)=>command==='tray_menu_state'
   ?{version:'0.2.35',mode:'tray',phase:'idle',videoBusy:false,update:null,theme:'plum',background:'',shortcuts:{recorder:['Ctrl','Shift','Espacio'],video:['Ctrl','Shift','F9'],capture:['Ctrl','Shift','F10'],library:['Alt','C']}}
   :original(command,args);
 });
 await menu.goto(base+'/tray-menu.html',{timeout:90000});
 await menu.getByRole('button',{name:/Configuración/}).waitFor();
 assert.equal(await prevented(menu,'.tm-card'),true,'Chrome menu is blocked inside the popup itself');
 // The keycaps are part of the tile: the longest combos stay on one line and inside its edges.
 const crooked=await menu.evaluate(()=>[...document.querySelectorAll('.tm-tile')].flatMap(tile=>{
  const box=tile.getBoundingClientRect(),keys=[...tile.querySelectorAll('kbd')].map(key=>key.getBoundingClientRect());
  return keys.some(r=>r.left<box.left||r.right>box.right)||new Set(keys.map(r=>Math.round(r.top))).size>1?[tile.textContent]:[];
 }));
 assert.deepEqual(crooked,[],'shortcut keys stay on one line inside their tile');
 assert.ok(await menu.evaluate(()=>window.calls.some(c=>c.command==='tray_menu_fit'&&c.args.mode==='tray')),'the page reports the height of the menu it draws');

 // Right-click on the folder: Rust opens the same window in folder mode.
 await menu.evaluate(()=>{window.calls.length=0;window.emitNative('tray-menu-open','folder','tray-menu');});
 await menu.getByRole('menu',{name:'Carpeta flotante'}).waitFor();
 assert.equal(await menu.locator('.tm-tile').count(),0,'no capture tiles on the folder menu');
 assert.deepEqual(await menu.locator('.tm-row-label').allTextContents(),['Portapapeles','Configuración']);
 await menu.waitForFunction(()=>window.calls.some(c=>c.command==='tray_menu_fit'&&c.args.mode==='folder'));
 await menu.getByRole('button',{name:/Configuración/}).click();
 assert.ok(await menu.evaluate(()=>window.calls.some(c=>c.command==='tray_menu_action'&&c.args.action==='settings')));

 // Closing empties the window, so the next menu never shows this one for a frame.
 await menu.evaluate(()=>window.emitNative('tray-menu-close',null,'tray-menu'));
 assert.equal(await menu.locator('.tm-card').count(),0);
 await menu.evaluate(()=>window.emitNative('tray-menu-open','tray','tray-menu'));
 await menu.locator('.tm-tile').nth(2).waitFor();
 assert.equal(await menu.locator('.tm-card[data-mode=folder]').count(),0);

 // The texts added here follow the system language, as the setup guide does: English wherever it is not Spanish.
 const englishFolder=await browser.newPage({viewport:{width:520,height:520},locale:'en-US'});
 await englishFolder.addInitScript(installNativeMock,{windowLabel:'recorder'});
 await englishFolder.addInitScript(()=>{const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args)=>command==='recording_state'?{phase:'idle',seconds:0,error:'No microphone',text:'',muted:false,progress:''}:command==='read_settings'?{color:'#9024DC',recorderScale:.85,pattern:'wave',dictationArtwork:'original',placement:'right'}:original(command,args);});
 await englishFolder.goto(base+'/?view=record',{timeout:90000});
 await englishFolder.getByRole('button',{name:'Close recorder',exact:true}).waitFor();
 const englishMenu=await browser.newPage({viewport:{width:344,height:480},locale:'en-US'});
 await englishMenu.addInitScript(installNativeMock,{windowLabel:'tray-menu'});
 await englishMenu.addInitScript(()=>{const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args)=>command==='tray_menu_state'?{version:'0.2.35',mode:'folder',phase:'idle',videoBusy:false,update:null,theme:'plum',background:'',shortcuts:{recorder:['Ctrl'],video:['Alt','X'],capture:['Alt','A'],library:['Alt','C']}}:original(command,args);});
 await englishMenu.goto(base+'/tray-menu.html',{timeout:90000});
 await englishMenu.evaluate(()=>window.emitNative('tray-menu-open','folder','tray-menu'));
 await englishMenu.getByRole('menu',{name:'Floating folder'}).waitFor();
 assert.deepEqual(await englishMenu.locator('.tm-row-label').allTextContents(),['Clipboard','Settings']);
 console.log('PASS: right-click on the folder asks for Whispera\'s menu and never Chrome\'s, the no-microphone notice has an X that closes the folder, and the popup is a short menu for the folder (clipboard, settings) or the full one for the tray, never both. Native IPC mocked.');
}finally{await browser.close();await server?.close();}
