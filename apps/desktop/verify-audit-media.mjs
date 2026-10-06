import {launchSilentBrowser, silencePage} from './tests/silent-browser.mjs';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,stat,writeFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {syntheticMedia} from './tests/synthetic-media.mjs';
const root=resolve('public/library');
const fixtures=syntheticMedia();
const server=createServer(async(req,res)=>{
  try{
    const url=new URL(req.url,'http://localhost');const path=fixtures[url.pathname]||resolve(root,'.'+decodeURIComponent(url.pathname));
    if(!fixtures[url.pathname]&&!path.startsWith(root+sep)){res.writeHead(403).end();return;}
    res.setHeader('Content-Type',{'.html':'text/html','.js':'text/javascript','.css':'text/css','.png':'image/png','.mp4':'video/mp4','.woff2':'font/woff2','.woff':'font/woff'}[extname(path)]||'application/octet-stream');
    const bytes=await readFile(path);
    if(req.headers.range){const [start,end]=req.headers.range.replace('bytes=','').split('-').map(Number);const to=end||bytes.length-1;res.writeHead(206,{'Accept-Ranges':'bytes','Content-Range':`bytes ${start}-${to}/${bytes.length}`,'Content-Length':to-start+1});res.end(bytes.subarray(start,to+1));}else res.end(bytes);
  }catch{res.writeHead(404).end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const base=`http://127.0.0.1:${server.address().port}`;
const browser=await launchSilentBrowser();
const errors=[];
async function pageFor(mode,id,width=1040,height=720){
  const page=await browser.newPage({viewport:{width,height}});page.setDefaultTimeout(12000);page.on('pageerror',e=>{errors.push(e.message);console.error('PAGE ERROR',e.message);});
  await silencePage(page);
  await page.addInitScript(()=>{
    const items=[{id:'image-1',data:{kind:'image',imageId:'/fixture-1.png',width:335,height:335,bytes:5000}}, {id:'image-2',data:{kind:'image',imageId:'/fixture-2.png',width:400,height:300,bytes:5000}}, {id:'video',data:{kind:'files',paths:['/fixture.mp4'],entries:[{name:'fixture.mp4',ext:'mp4',isImage:false}]}}];
    window.__qa={calls:[],events:new Map()};
    window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'media-canvas-test'}},convertFileSrc:path=>path,transformCallback:fn=>fn,invoke:async(command,args={})=>{
      window.__qa.calls.push({command,args});
      if(command==='library_state')return{items,settings:{},revision:'1'};
      if(command==='library_media_info')return{previewPath:'/poster.png',duration:12.7,video:true};
      if(command==='library_pick')return false; // User cancels the native picker; no data changes.
      if(command==='plugin:event|listen'){window.__qa.events.set(args.event,args.handler);return 1;}
      return true;
    }};
  });
  await page.goto(`${base}/media.html?mode=${mode}&id=${id}`);await page.locator('.media-surface.ready').waitFor();await page.waitForTimeout(450);return page;
}
try{
  const canvas=await pageFor('canvas','image-1');
  await canvas.locator('.composition-layer img').first().waitFor();
  assert.ok(await canvas.locator('.composition-layer img').evaluate(img=>img.naturalWidth>0));
  await canvas.locator('.composition-background').waitFor();
  assert.equal(await canvas.locator('.gradient-swatches button').count(),12);
  await canvas.screenshot({path:resolve('../../.local/editor-backgrounds-headless.png')});
  const beforePanel=(await canvas.locator('.composition-stage').boundingBox()).width;
  await canvas.getByRole('button',{name:'Ocultar ajustes',exact:true}).click();
  await canvas.waitForTimeout(400);
  assert.ok((await canvas.locator('.composition-stage').boundingBox()).width>=beforePanel);
  await canvas.getByRole('button',{name:'Mostrar ajustes',exact:true}).click();
  await canvas.waitForTimeout(400);
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  await canvas.locator('.capture-picker button[title="Agregar al lienzo"]').nth(1).click();
  await canvas.waitForFunction(()=>document.querySelectorAll('.composition-layer').length===2);
  assert.equal(await canvas.locator('.composition-layer').count(),2);
  const layer=canvas.locator('.composition-layer').first();const old=await layer.boundingBox();
  await canvas.mouse.move(old.x+old.width/2,old.y+old.height/2);await canvas.mouse.down();await canvas.mouse.move(old.x+old.width/2+32,old.y+old.height/2+15,{steps:5});await canvas.mouse.up();
  assert.ok((await layer.boundingBox()).x>old.x+20);
  const handle=canvas.getByRole('button',{name:'Redimensionar captura'});const grip=await handle.boundingBox();const previousWidth=(await layer.boundingBox()).width;
  await canvas.mouse.move(grip.x+4,grip.y+4);await canvas.mouse.down();await canvas.mouse.move(grip.x+25,grip.y+4,{steps:4});await canvas.mouse.up();await canvas.waitForTimeout(80);console.log('Resize',previousWidth,(await layer.boundingBox()).width);assert.ok((await layer.boundingBox()).width>previousWidth);
  await canvas.getByRole('button',{name:'Dorado',exact:true}).click();
  await canvas.getByRole('button',{name:'Guardar y copiar'}).click();await canvas.waitForFunction(()=>window.__qa.calls.some(c=>c.command==='library_media_export'));
  const png=await canvas.evaluate(()=>window.__qa.calls.find(c=>c.command==='library_media_export').args.png);
  await writeFile(resolve('../../.local/media-composition-export.png'),Buffer.from(png.split(',')[1],'base64'));
  const pixel=await canvas.evaluate(async(png)=>{const img=new Image();img.src=png;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;const ctx=c.getContext('2d');ctx.drawImage(img,0,0);return{width:img.width,height:img.height,corner:[...ctx.getImageData(0,0,1,1).data]};},png);
  assert.deepEqual(pixel,{width:960,height:600,corner:[229,179,34,255]});
  await canvas.getByRole('button',{name:'Proporcion',exact:true}).click();
  await canvas.getByRole('menuitemradio',{name:'9:16',exact:true}).click();await canvas.waitForTimeout(250);
  const box=await canvas.locator('.composition-stage').boundingBox();assert.ok(Math.abs(box.width/box.height-9/16)<.002);
  assert.ok(box.y+box.height<720);
  await canvas.getByRole('button',{name:'Proporcion',exact:true}).click();
  await canvas.getByRole('menuitemradio',{name:'Original',exact:true}).click();
  await canvas.getByRole('switch',{name:'Equilibrado automatico'}).click();
  assert.equal(await canvas.getByRole('switch',{name:'Equilibrado automatico'}).getAttribute('aria-checked'),'false');
  await canvas.getByRole('switch',{name:'Equilibrado automatico'}).click();
  await canvas.getByRole('slider',{name:'Esquinas'}).fill('24');
  await canvas.getByRole('button',{name:'Proporcion',exact:true}).click();
  await canvas.keyboard.press('Escape');
  await canvas.waitForTimeout(200);
  assert.equal(await canvas.getByRole('menu').count(),0);
  assert.ok(!await canvas.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_media_window'&&c.args.action==='close')),'Escape closes menu, not editor');
  await canvas.emulateMedia({reducedMotion:'reduce'});
  await canvas.getByRole('button',{name:'Proporcion',exact:true}).click();
  assert.equal(await canvas.getByRole('menu').evaluate(el=>getComputedStyle(el).transitionDuration),'0s');
  await canvas.keyboard.press('ArrowDown');
  await canvas.screenshot({path:resolve('../../.local/editor-controls-headless.png')});
  await canvas.keyboard.press('Escape');
  await canvas.emulateMedia({reducedMotion:'no-preference'});
  await canvas.screenshot({path:resolve('../../.local/media-canvas-headless.png')});
  await canvas.getByRole('button',{name:'Transparente',exact:true}).click();await canvas.getByRole('button',{name:'Guardar y copiar'}).click();
  await canvas.waitForFunction(()=>window.__qa.calls.filter(c=>c.command==='library_media_export').length===2);
  assert.ok(await canvas.evaluate(async()=>{const png=window.__qa.calls.filter(c=>c.command==='library_media_export').at(-1).args.png;const i=new Image();i.src=png;await i.decode();const c=document.createElement('canvas');c.width=i.width;c.height=i.height;const ctx=c.getContext('2d');ctx.drawImage(i,0,0);return ctx.getImageData(0,0,1,1).data[3]===0;}));
  assert.equal(await canvas.locator('.media-preview,.quick-actions,.float-settings').count(),0);
  for(const [name,count] of [['Fondo Iris',3],['Difuminado',4]]){
    const previous=await canvas.locator('.composition-background').getAttribute('src');
    await canvas.getByRole('button',{name,exact:true}).click();
    await canvas.waitForFunction(previous=>document.querySelector('.composition-background').getAttribute('src')!==previous,previous);
    await canvas.getByRole('button',{name:'Guardar y copiar'}).click();
    await canvas.waitForFunction(count=>window.__qa.calls.filter(c=>c.command==='library_media_export').length===count,count);
    assert.ok(await canvas.evaluate(async()=>{
      const png=window.__qa.calls.filter(c=>c.command==='library_media_export').at(-1).args.png;
      const sample=async src=>{const image=new Image();image.src=src;await image.decode();const c=document.createElement('canvas');c.width=image.width;c.height=image.height;const ctx=c.getContext('2d');ctx.drawImage(image,0,0);return [...ctx.getImageData(0,0,1,1).data];};
      const preview=await sample(document.querySelector('.composition-background').src),exported=await sample(png);
      return exported[3]===255&&preview.every((n,i)=>Math.abs(n-exported[i])<=2);
    }),'Background preview/export pixels must agree');
  }
  await canvas.getByRole('button',{name:'Elegir imagen',exact:true}).click();
  await canvas.getByTitle('Usar como fondo',{exact:true}).first().click();
  assert.equal(await canvas.getByRole('button',{name:'Elegir imagen',exact:true}).getAttribute('aria-pressed'),'true');
  await canvas.getByRole('button',{name:'Fondo Iris',exact:true}).click();
  await canvas.setViewportSize({width:600,height:620});await canvas.waitForTimeout(350);
  assert.ok(await canvas.locator('.media-surface').evaluate(el=>el.scrollWidth<=el.clientWidth+1));
  await canvas.screenshot({path:resolve('../../.local/editor-backgrounds-compact-headless.png')});
  const fromVideo=await pageFor('canvas','video');
  assert.equal(await fromVideo.locator('.composition-layer').count(),0);
  await fromVideo.getByRole('button',{name:'Agregar captura',exact:true}).click();
  await fromVideo.locator('.capture-picker button[title="Agregar al lienzo"]').first().click();
  await fromVideo.locator('.composition-layer').waitFor();
  await fromVideo.close();

  await canvas.setViewportSize({width:1040,height:720});
  const controls=await canvas.locator('button,input,select').evaluateAll(es=>es.filter(e=>e.getClientRects().length).map(e=>({tag:e.tagName,label:e.getAttribute('aria-label')||e.title||e.textContent?.trim(),role:e.getAttribute('role'),min:e.min,max:e.max,disabled:e.disabled})));
  const matrix=[];
  for(const [label,ratio] of [['Original',1.6],['16:9',16/9],['1:1',1],['4:5',4/5],['9:16',9/16]]){
    await canvas.getByRole('button',{name:'Proporcion',exact:true}).click();await canvas.getByRole('menuitemradio',{name:label,exact:true}).click();
    await canvas.waitForTimeout(200);const rect=await canvas.locator('.composition-stage').boundingBox();assert.ok(Math.abs(rect.width/rect.height-ratio)<.003);matrix.push({control:'Proporcion',value:label,passed:true});
  }
  await canvas.getByRole('button',{name:'Proporcion',exact:true}).click();await canvas.getByRole('menuitemradio',{name:'Original',exact:true}).click();
  for(const [label,lo,hi] of [['Margen',0,160],['Esquinas',0,96],['Sombra',0,70]]){
    for(const value of [lo,Math.round((lo+hi)/2),hi]){await canvas.getByRole('slider',{name:label,exact:true}).fill(String(value));assert.equal(await canvas.getByRole('slider',{name:label,exact:true}).inputValue(),String(value));matrix.push({control:label,value,passed:true});}
  }
  for(const button of await canvas.locator('.gradient-swatches button,.background-swatches button').all()){
    const name=await button.getAttribute('aria-label');await button.click();await canvas.waitForTimeout(60);assert.equal(await button.getAttribute('aria-pressed'),'true');
    const count=await canvas.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_media_export').length);
    await canvas.locator('.export-btn').click();await canvas.waitForFunction(count=>window.__qa.calls.filter(c=>c.command==='library_media_export').length>count,count);
    assert.ok(await canvas.evaluate(async()=>{const src=window.__qa.calls.filter(c=>c.command==='library_media_export').at(-1).args.png;const i=new Image();i.src=src;await i.decode();return i.width===960&&i.height===600;}));
    matrix.push({control:'Fondo',value:name,passed:true,effect:'PNG decodable 960x600'});
  }
  await canvas.getByRole('button',{name:'Difuminado',exact:true}).click();
  for(const value of [4,32,64]){await canvas.getByRole('slider',{name:'Desenfoque',exact:true}).fill(String(value));assert.equal(await canvas.getByRole('slider',{name:'Desenfoque',exact:true}).inputValue(),String(value));matrix.push({control:'Desenfoque',value,passed:true});}
  await canvas.getByRole('button',{name:'Equilibrar',exact:true}).click();
  await canvas.getByRole('button',{name:'Ocultar ajustes',exact:true}).click();await canvas.getByRole('button',{name:'Mostrar ajustes',exact:true}).waitFor();await canvas.getByRole('button',{name:'Mostrar ajustes',exact:true}).click();
  const count=await canvas.locator('.composition-layer').count();await canvas.getByRole('button',{name:'Captura 1',exact:true}).click();
  await canvas.getByRole('button',{name:'Traer adelante',exact:true}).click();await canvas.getByRole('button',{name:'Enviar atras',exact:true}).click();await canvas.getByRole('button',{name:'Quitar captura',exact:true}).click();assert.equal(await canvas.locator('.composition-layer').count(),count-1);
  matrix.push({control:'Equilibrar, ocultar/mostrar panel, seleccionar/ordenar/quitar capa',passed:true});
  const layersBeforeCancel=await canvas.locator('.composition-layer img').evaluateAll(images=>images.map(image=>image.src));
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  await canvas.locator('.capture-picker').waitFor();
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  assert.equal(await canvas.locator('.capture-picker').count(),0);
  assert.deepEqual(await canvas.locator('.composition-layer img').evaluateAll(images=>images.map(image=>image.src)),layersBeforeCancel);
  matrix.push({control:'Cancelar selector de captura',value:'Cerrar selector interno sin elegir',passed:true,effect:'Capas intactas'});
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  await canvas.getByRole('button',{name:'Importar',exact:true}).click();
  await canvas.waitForFunction(()=>window.__qa.calls.some(call=>call.command==='library_pick'));
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  assert.deepEqual(await canvas.locator('.composition-layer img').evaluateAll(images=>images.map(image=>image.src)),layersBeforeCancel);
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  await canvas.locator('.capture-picker').waitFor();
  await canvas.getByRole('button',{name:'Agregar captura',exact:true}).click();
  matrix.push({control:'Cancelar selector de captura',value:'library_pick devuelve cancelación; reabrir funciona',passed:true,effect:'Adaptador real; diálogo Windows simulado; capas intactas'});
  await canvas.getByLabel('Color personalizado',{exact:true}).fill('#123456');
  const exportCount=await canvas.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_media_export').length);
  await canvas.locator('.export-btn').click();
  await canvas.waitForFunction(n=>window.__qa.calls.filter(c=>c.command==='library_media_export').length>n,exportCount);
  const customPixel=await canvas.evaluate(async()=>{const i=new Image();i.src=window.__qa.calls.filter(c=>c.command==='library_media_export').at(-1).args.png;await i.decode();const c=document.createElement('canvas');c.width=i.width;c.height=i.height;const x=c.getContext('2d');x.drawImage(i,0,0);return [...x.getImageData(0,0,1,1).data];});
  assert.deepEqual(customPixel,[18,52,86,255]);matrix.push({control:'Color personalizado',value:'#123456',passed:true,effect:'PNG exported corner RGBA 18,52,86,255'});
  await writeFile(resolve('../../outputs/auditoria-2026-10-06/evidencias/media-options.json'),JSON.stringify({controls,matrix},null,2));

  await canvas.getByRole('button',{name:'Cerrar',exact:true}).click();
  await canvas.waitForFunction(()=>window.__qa.calls.some(c=>c.command==='library_media_window'&&c.args.action==='close'));
  assert.deepEqual(errors,[]);
  console.log('PASS silent headless: two-image canvas, move/resize, gold PNG pixels, transparent alpha, portrait aspect, close IPC, no floating overlay. No audio playback or native Windows UI.');
}finally{await browser.close();await new Promise(r=>server.close(r));}
