import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {launchSilentBrowser, silencePage} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const root=resolve('dist');
const server=createServer(async(req,res)=>{try{const url=new URL(req.url,'http://localhost');const path=resolve(root,'.'+(url.pathname==='/'?'/index.html':url.pathname));if(!path.startsWith(root+sep)){res.writeHead(403).end();return;}res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.woff2':'font/woff2'})[extname(path)]||'application/octet-stream');res.end(await readFile(path));}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await launchSilentBrowser();
function installLibraryMock() {
 const fixture=(id,data,pinned=false)=>({id,data,pinned,capturedAt:1790985600000,hitCount:1});
 window.libraryItems=[
  fixture('text',{kind:'text',text:'Nota de biblioteca'}),
  fixture('link',{kind:'text',text:'https://example.test/referencia'}),
  fixture('image',{kind:'image',imageId:'C:\\QA\\captura.png'}),
  fixture('collection',{kind:'image-collection',images:[{imageId:'C:\\QA\\uno.png'},{imageId:'C:\\QA\\dos.png'}]}),
  fixture('video',{kind:'files',paths:['C:\\QA\\grabacion.MP4'],entries:[{name:'grabacion.MP4'}]}),
  fixture('file',{kind:'files',paths:['C:\\QA\\informe.pdf'],entries:[{name:'informe.pdf'}]},true),
  fixture('mixed',{kind:'files',paths:['C:\\QA\\clip.mp4','C:\\QA\\adjunto.txt'],entries:[{name:'clip.mp4'},{name:'adjunto.txt'}]}),
  fixture('long',{kind:'text',text:'palabra'.repeat(100)}),
 ];
 window.libraryRevision=1;
 const original=window.__TAURI_INTERNALS__.invoke;
 window.__TAURI_INTERNALS__.convertFileSrc=()=> 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=';
 window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
  if(command==='library_state'||command==='library_toggle'||command==='library_media_open'||(command==='library_action'&&args.action!=='settings')){
   window.calls.push({command,args});
   if(command==='library_state'){
    if(window.failLibraryRead)throw Error('Lectura no disponible');
    const revision=String(window.libraryRevision);
    return args.revision===revision?{unchanged:true}:{items:structuredClone(window.libraryItems),revision};
   }
   if(window.failLibraryAction)throw Error('Acción no disponible');
   if(command==='library_action'){
    if(args.action==='pin')window.libraryItems.find(item=>item.id===args.id).pinned=args.value;
    if(args.action==='delete')window.libraryItems=window.libraryItems.filter(item=>item.id!==args.id);
    if(['pin','delete'].includes(args.action))window.libraryRevision++;
   }
   return true;
  }
  return original(command,args);
 };
}
try{
 const page=await browser.newPage({viewport:{width:1100,height:800}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await silencePage(page);
 await page.addInitScript({content:`(${installNativeMock.toString()})({});(${installLibraryMock.toString()})();`});
 await page.goto(process.env.LIBRARY_TEST_URL||`http://127.0.0.1:${server.address().port}/`);
 await page.getByRole('button',{name:'Portapapeles',exact:true}).click();
 await page.locator('[data-library-id="text"]').waitFor();
 assert.equal(await page.getByRole('tab',{name:'Biblioteca',exact:true}).getAttribute('aria-selected'),'true');
 const rows=page.locator('.library-item');
 assert.equal(await rows.count(),8);
 assert.equal(await rows.first().getAttribute('data-library-id'),'file');
 await page.screenshot({path:'../../.local/library-browser-headless.png'});
 for(const [name,count] of [['Textos',2],['Enlaces',1],['Imágenes',2],['Videos',1],['Archivos',2],['Todos',8]]){
  await page.getByRole('button',{name,exact:true}).click();assert.equal(await rows.count(),count,name);
 }
 const search=page.getByRole('searchbox',{name:'Buscar en biblioteca'});
 await search.fill('INFORME');assert.equal(await rows.count(),1);
 await search.fill('sin coincidencia de prueba');await page.getByText('Sin coincidencias.',{exact:true}).waitFor();
 await search.fill('');
 const row=id=>page.locator(`[data-library-id="${id}"]`);
 const waitCall=(command,action)=>page.waitForFunction(({command,action})=>window.calls.some(call=>call.command===command&&(!action||call.args.action===action)),{command,action});
 await row('text').getByRole('button',{name:'Copiar',exact:true}).click();await page.getByText('Copiado.',{exact:true}).waitFor();
 await row('text').getByRole('button',{name:'Fijar',exact:true}).click();await row('text').getByRole('button',{name:'Desfijar',exact:true}).waitFor();
 await row('text').getByRole('button',{name:'Desfijar',exact:true}).click();await row('text').getByRole('button',{name:'Fijar',exact:true}).waitFor();
 await row('image').getByRole('button',{name:'Editar imagen',exact:true}).click();await waitCall('library_media_open');
 assert.deepEqual(await page.evaluate(()=>window.calls.find(call=>call.command==='library_media_open').args),{id:'image',mode:'canvas'});
 await row('file').getByRole('button',{name:'Abrir ubicación',exact:true}).click();await waitCall('library_action','reveal');
 assert.equal(await row('text').getByRole('button',{name:'Abrir ubicación',exact:true}).count(),0);
 for(const id of ['video','text','link','file']){
  assert.equal(await row(id).getByRole('button',{name:'Editar imagen',exact:true}).count(),0);
 }
 await page.getByRole('button',{name:'Abrir estante',exact:true}).click();await waitCall('library_toggle');
 await row('text').getByRole('button',{name:'Eliminar del historial',exact:true}).click();
 await row('text').getByRole('button',{name:'Cancelar',exact:true}).click();
 assert.equal(await page.evaluate(()=>window.calls.filter(call=>call.command==='library_action'&&call.args.action==='delete').length),0);
 await row('text').getByRole('button',{name:'Eliminar del historial',exact:true}).click();
 await row('text').getByRole('button',{name:'Eliminar',exact:true}).click();await row('text').waitFor({state:'detached'});
 await page.evaluate(()=>{window.failLibraryAction=true;});
 await row('link').getByRole('button',{name:'Copiar',exact:true}).click();await page.getByRole('alert').filter({hasText:'Acción no disponible'}).waitFor();
 await page.evaluate(()=>{window.failLibraryAction=false;});
 await row('link').getByRole('button',{name:'Copiar',exact:true}).click();await page.getByText('Copiado.',{exact:true}).waitFor();
 await page.evaluate(()=>{window.failLibraryRead=true;});
 await page.getByRole('button',{name:'Actualizar biblioteca'}).click();await page.getByRole('alert').filter({hasText:'Lectura no disponible'}).waitFor();
 await page.evaluate(()=>{window.failLibraryRead=false;});
 await page.getByRole('button',{name:'Actualizar biblioteca'}).click();await page.getByRole('alert').waitFor({state:'detached'});
 for(const width of [800,1100]){
  await page.setViewportSize({width,height:800});
  assert.ok(await page.locator('.library-browser').evaluate(element=>element.scrollWidth<=element.clientWidth+1),'Library must not overflow');
  await page.screenshot({path:`../../.local/library-browser-${width}-headless.png`});
 }
 await page.getByRole('tab',{name:'Configuración',exact:true}).click();
 await page.locator('#library-limit').fill('120');
 await page.getByRole('tab',{name:'Biblioteca',exact:true}).click();
 await page.getByRole('tab',{name:'Configuración',exact:true}).click();
 assert.equal(await page.locator('#library-limit').inputValue(),'120','Unsaved settings survive tab switches');
 await page.locator('#library-retention').selectOption('0');
 assert.equal(await page.locator('#library-captureGlobal').getAttribute('aria-checked'),'true');
 await page.getByRole('button',{name:'Guardar',exact:true}).click();
 await page.waitForFunction(()=>window.libraryPreferences.historyLimit===120);
 assert.equal(await page.evaluate(()=>window.libraryPreferences.autoDeleteHours),0);
 assert.equal(await page.evaluate(()=>window.libraryPreferences.transcribeVideo),true);
 const patch=await page.evaluate(()=>window.calls.find(c=>c.command==='library_action'&&c.args.action==='settings').args.value);assert.ok(!('toggleHotkey' in patch));
 await page.getByRole('tab',{name:'Configuración',exact:true}).focus();await page.keyboard.press('ArrowLeft');
 assert.equal(await page.getByRole('tab',{name:'Biblioteca',exact:true}).getAttribute('aria-selected'),'true');
 await page.evaluate(()=>{window.libraryItems=[];window.libraryRevision++;});
 await page.getByRole('button',{name:'Actualizar biblioteca'}).click();await page.getByText('La biblioteca está vacía.',{exact:true}).waitFor();
 await page.evaluate(()=>{window.libraryItems=Array.from({length:65},(_,i)=>({id:`page-${i}`,pinned:false,capturedAt:1790985600000,data:{kind:'text',text:`Nota ${i}`}}));window.libraryRevision++;});
 await page.getByRole('button',{name:'Actualizar biblioteca'}).click();await page.getByRole('button',{name:'Mostrar más',exact:true}).waitFor();
 assert.equal(await rows.count(),50);await page.getByRole('button',{name:'Mostrar más',exact:true}).click();assert.equal(await rows.count(),65);
 await page.getByRole('button',{name:'Atajos',exact:true}).click();
 const key=page.locator('#shortcut-library');await key.click();await page.waitForFunction(()=>document.querySelector('#shortcut-library').parentElement.dataset.listening==='true');
 await page.keyboard.press('Control+Alt+KeyB');await page.keyboard.press('Tab');
 await page.getByRole('button',{name:'Guardar',exact:true}).click();
 await page.getByText('Los cuatro atajos quedaron guardados.').waitFor();
 assert.equal(await page.evaluate(()=>window.libraryPreferences.toggleHotkey),'Control+Alt+KeyB');
 assert.ok(!await page.evaluate(()=>window.calls.some(c=>['screen_start','screen_select','library_drag','library_collect','library_pick'].includes(c.command))));assert.deepEqual(errors,[]);
 console.log('PASS: library tabs, real-API contract mocks, filters, search, copy/pin/delete/reveal/canvas/shelf, errors, pagination, settings persistence and shortcuts. Silent headless; no native effects.');
}finally{await browser.close();await new Promise(r=>server.close(r));}
