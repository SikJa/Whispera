import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});await server?.listen();
const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
const browser=await launchSilentBrowser();
try{
 const page=await browser.newPage({viewport:{width:760,height:960}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{
   window.menu={version:'0.3.0-readaloud.11',phase:'idle',videoBusy:false,theme:'plum',background:'',update:null,shortcuts:{recorder:['Ctrl','Space'],video:['Alt','X'],capture:['Alt','A'],library:['Alt','C'],reading:['Ctrl','Alt','R']},reader:{supported:true,phase:'idle',exporting:false,has_reading:false}};
   const old=window.__TAURI_INTERNALS__.invoke;
   window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
     if(command==='tray_menu_state')return structuredClone(window.menu);
     if(command==='tray_menu_action'){window.calls.push({command,args});return;}
     return old(command,args);
   };
 });
 await page.goto(base+'/tray-menu.html',{timeout:90000,waitUntil:'domcontentloaded'});
 const pause=page.getByRole('button',{name:'Pausar lectura',exact:true});await pause.waitFor();assert.ok(await pause.isDisabled());
 assert.ok(await page.getByRole('button',{name:'Detener lectura',exact:true}).isDisabled());
 assert.ok(await page.getByRole('button',{name:'Guardar lectura como MP3…',exact:true}).isDisabled());
 await page.evaluate(()=>{window.menu.reader={supported:true,phase:'speaking',exporting:false,has_reading:true};window.emitNative('tray-menu-open',null);});
 await page.waitForFunction(()=>[...document.querySelectorAll('button')].some(e=>e.textContent==='Pausar lectura'&&!e.disabled));
 await pause.click();assert.equal(await page.evaluate(()=>window.calls.at(-1).args.action),'reader-pause');
 await page.evaluate(()=>{window.menu.reader.phase='paused';window.emitNative('reader-tray-state',null);});
 await page.getByRole('button',{name:'Continuar lectura',exact:true}).click();
 assert.equal(await page.evaluate(()=>window.calls.at(-1).args.action),'reader-pause');
 for(const [name,action] of [['Detener lectura','reader-stop'],['Guardar lectura como MP3…','reader-export'],['Uso de IA','ai-usage'],['Lectura en voz alta','reading']]){
   await page.getByRole('button',{name:name==='Lectura en voz alta'?/^Lectura en voz alta/:name,exact:true}).click();assert.equal(await page.evaluate(()=>window.calls.at(-1).args.action),action);
 }
 await page.evaluate(()=>{window.menu.reader.exporting=true;window.emitNative('reader-tray-state',null);});
 await page.getByRole('button',{name:'Cancelar guardado MP3',exact:true}).click();
 assert.equal(await page.evaluate(()=>window.calls.at(-1).args.action),'reader-export-cancel');
 assert.ok(await page.getByRole('button',{name:'Guardando MP3…',exact:true}).isDisabled());
 await page.setViewportSize({width:360,height:520});
 assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
 await page.getByRole('button',{name:'Salir de Whispera',exact:true}).scrollIntoViewIfNeeded();
 assert.ok(await page.getByRole('button',{name:'Salir de Whispera',exact:true}).isVisible());
 const settings=await browser.newPage();settings.on('pageerror',e=>errors.push(e.message));await settings.addInitScript(installNativeMock,{});
 await settings.addInitScript(()=>{
   const old=window.__TAURI_INTERNALS__.invoke;
   window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
     if(command==='local_models')return {models:[{id:'parakeet-v3',name:'Parakeet v3',description:'Modelo de prueba',bytes:739508576,installed:false}],install:{phase:'',model:'',downloaded:0,total:0,error:''},loaded:'',device:'',error:''};
     if(command==='audio_input_devices')return ['Micrófono de prueba'];
     return old(command,args);
   };
 });
 await settings.goto(base,{timeout:90000,waitUntil:'domcontentloaded'});
 const search=settings.getByRole('combobox',{name:'Buscar opciones'});
 await search.fill('microfono dictado');await settings.getByRole('option').filter({has:settings.getByText('Micrófono de dictado',{exact:true})}).click();
 await settings.locator('#microphone').waitFor();
 await search.fill('parakeet');await settings.getByRole('option').filter({has:settings.getByText('Motor local o Groq',{exact:true})}).click();
 await settings.getByRole('radio',{name:/Local/}).click();
 await settings.getByText(/La instalación incluye un detector de voz de 1,2 MB/).waitFor();
 await settings.getByText(/En Windows necesita un controlador compatible con Vulkan/).waitFor();
 assert.equal(await settings.evaluate(()=>window.calls.some(c=>c.command==='local_model_install')),false,'Selecting local must not download a model');
 assert.deepEqual(errors,[]);
 console.log('PASS: new tray preserves reader pause/resume/stop, MP3/cancel, quota and reader links; small tray scrolls; local-engine/microphone searchable; selecting local does not install models. IPC mocked, no downloads or audio.');
}finally{await browser.close();await server?.close();}
