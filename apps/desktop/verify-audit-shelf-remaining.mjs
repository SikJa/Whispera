import {launchSilentBrowser,silencePage} from './tests/silent-browser.mjs';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
const root=resolve('public/library');
const server=createServer(async(req,res)=>{try{const p=resolve(root,'.'+new URL(req.url,'http://localhost').pathname);if(!p.startsWith(root+sep)){res.writeHead(403).end();return;}res.setHeader('Content-Type',{'.html':'text/html','.js':'text/javascript','.css':'text/css'}[extname(p)]||'application/octet-stream');res.end(await readFile(p));}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await launchSilentBrowser();const results=[];
async function fresh(){
 const page=await browser.newPage({viewport:{width:1280,height:800}});await silencePage(page);page.setDefaultTimeout(12000);page.on('pageerror',e=>{throw e;});
 await page.addInitScript(()=>{
  const now=Date.now();let revision=1;const items=[['pinned',.1,true],['recent',.25,false],['three',3,false],['twelve',12,false],['old',30,false]].map(([id,hours,pinned])=>({id,data:{kind:'text',text:'Sintético '+id,isUrl:false},capturedAt:now-hours*3600000,hitCount:1,pinned}));
  items.push({id:'file',data:{kind:'files',paths:['C:\\Auditoría ñ\\fixture.pdf'],entries:[{name:'fixture.pdf',ext:'pdf',size:100,isImage:false}]},capturedAt:now-30*3600000,hitCount:1,pinned:false});
  const settings={captureGlobal:true,tutorialCompleted:true,hoverActivation:false,language:'es',stickPosition:'left',soundEffects:false,verticalOffset:.5,panelHeight:.9};
  window.__qa={calls:[],items,settings,failDelete:false};
  window.__TAURI_INTERNALS__={convertFileSrc:x=>x,transformCallback:fn=>fn,invoke:async(command,args={})=>{
   window.__qa.calls.push({command,args});
   if(command==='library_state')return args.revision===String(revision)?{unchanged:true}:{items:structuredClone(items),settings:{...settings},revision:String(revision),version:'0.2.22',isStoreBuild:false};
   if(command==='library_action'){
    if(window.__qa.failDelete&&['clear','delete'].includes(args.action))throw Error('Fallo simulado; conservar datos');
    if(args.action==='settings')Object.assign(settings,args.value);
    if(args.action==='pin')items.find(i=>i.id===args.id).pinned=args.value;
    if(args.action==='delete'){const i=items.findIndex(i=>i.id===args.id);if(i>=0)items.splice(i,1);}
    if(args.action==='clear'){for(let i=items.length-1;i>=0;i--)if(!items[i].pinned)items.splice(i,1);}
    revision++;return structuredClone(settings);
   }
   return true;
  }};
 });
 await page.goto(`http://127.0.0.1:${server.address().port}/browser-preview.html`);await page.waitForFunction(()=>document.querySelectorAll('.item-main').length===5);await page.waitForTimeout(400);return page;
}
try{
 for(const [label,expected] of [['Borrar la última hora',['recent']],['Borrar las últimas 6 horas',['recent','three']],['Borrar las últimas 24 horas',['recent','three','twelve']]]){
  const p=await fresh();await p.locator('.footer .text-btn').click();await p.getByRole('button',{name:label,exact:true}).click();
  await p.waitForFunction(n=>window.__qa.items.length===6-n,expected.length);
  const deleted=await p.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_action'&&c.args.action==='delete').map(c=>c.args.id));assert.deepEqual(deleted.sort(),expected.sort());
  assert.ok(await p.evaluate(()=>window.__qa.items.some(i=>i.id==='pinned')));results.push({case:label,deleted,keptPinned:true,passed:true});await p.close();
 }
 {
  const p=await fresh();await p.locator('.footer .text-btn').click();await p.getByRole('button',{name:'Borrar todo el historial',exact:true}).click();
  assert.equal(await p.evaluate(()=>window.__qa.items.length),6);await p.getByRole('button',{name:'Toca de nuevo para confirmar',exact:true}).click();await p.waitForFunction(()=>window.__qa.items.length===1);
  assert.equal(await p.evaluate(()=>window.__qa.items[0].id),'pinned');results.push({case:'Borrar todo exige segunda confirmación y conserva fijados',passed:true});await p.close();
 }
 {
  const p=await fresh();await p.evaluate(()=>window.__qa.failDelete=true);await p.locator('.footer .text-btn').click();await p.getByRole('button',{name:'Borrar la última hora',exact:true}).click();
  await p.waitForTimeout(250);assert.equal(await p.locator('.item-main').count(),5);assert.equal(await p.evaluate(()=>window.__qa.items.length),6);
  await p.evaluate(()=>window.__qa.failDelete=false);await p.locator('.footer .text-btn').click();await p.getByRole('button',{name:'Borrar la última hora',exact:true}).click();await p.waitForFunction(()=>window.__qa.items.length===5);
  results.push({case:'Fallo al borrar restaura elementos; reintento funciona',passed:true});await p.close();
 }
 {
  const p=await fresh();await p.locator('.header button[title="Configuración"]').click();await p.getByRole('button',{name:'Posición',exact:true}).click();
  const slider=p.getByRole('slider',{name:'Posición vertical',exact:true});await slider.scrollIntoViewIfNeeded();const track=slider.locator('..');const box=await track.boundingBox();
  await p.mouse.move(box.x+box.width*.5,box.y+box.height*.5);await p.mouse.down();await p.mouse.move(box.x+box.width*.75,box.y+box.height*.5,{steps:10});await p.mouse.up();
  await p.waitForFunction(()=>window.__qa.settings.verticalOffset>.7&&window.__qa.settings.verticalOffset<.8);assert.ok(Number(await slider.getAttribute('aria-valuenow'))>.7);
  for(const [key,value] of [['Home',0],['End',1]]){await slider.focus();await p.keyboard.press(key);await p.waitForFunction(v=>window.__qa.settings.verticalOffset===v,value);}
  results.push({case:'Slider arrastre y Home/End persisten, límites 0/1',passed:true});
  await p.locator('.header button[title="Cerrar"]').click();await p.locator('.header button[title="Configuración"]').waitFor();results.push({case:'Cerrar configuración del estante vuelve a historial',passed:true});
  await p.locator('.item-main[data-id="file"]').hover();await p.locator('.item-main[data-id="file"] .preview-expand').click();await p.waitForTimeout(350);
  const reveal=await p.locator('.preview-flyout button').evaluateAll(es=>es.map(x=>({title:x.title,text:x.textContent})));console.log('FILE_PREVIEW_BUTTONS',JSON.stringify(reveal));
  const explorer=p.getByRole('button',{name:/explorador|Explorer|ubicación/i});await explorer.first().click();await p.waitForFunction(()=>window.__qa.calls.some(c=>c.command==='library_action'&&c.args.action==='reveal'));
  const request=await p.evaluate(()=>window.__qa.calls.find(c=>c.command==='library_action'&&c.args.action==='reveal').args);assert.equal(request.id,'file');assert.deepEqual(request.value.paths,['C:\\Auditoría ñ\\fixture.pdf']);results.push({case:'Abrir ubicación transmite archivo Unicode seleccionado al comando real; Explorer simulado',passed:true});
  await p.keyboard.press('Escape');await p.waitForFunction(()=>window.__qa.calls.some(c=>c.command==='library_window'&&c.args.operation==='hide'));results.push({case:'Escape solicita ocultar ventana nativa',passed:true});await p.close();
 }
 await writeFile('../../outputs/auditoria-2026-10-06/evidencias/shelf-remaining.json',JSON.stringify({method:'React real y adaptador compilado; IPC simulado, sin perfil personal',results},null,2));console.log('PASS remaining shelf',results.length);
}finally{await browser.close();await new Promise(r=>server.close(r));}
