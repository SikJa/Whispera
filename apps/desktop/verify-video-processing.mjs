import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
const root=resolve('public/library');
const server=createServer(async(req,res)=>{try{const path=resolve(root,'.'+new URL(req.url,'http://localhost').pathname);if(!path.startsWith(root+sep)){res.writeHead(403).end();return;}res.setHeader('Content-Type',{'.html':'text/html','.js':'text/javascript','.css':'text/css','.woff2':'font/woff2','.woff':'font/woff'}[extname(path)]||'application/octet-stream');res.end(await readFile(path));}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await launchSilentBrowser();
try{
 const page=await browser.newPage({viewport:{width:1280,height:800}});
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(()=>{
  const pixel='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=';
  window.qa={calls:[],events:new Map(),items:[],revision:1};
  window.__TAURI_INTERNALS__={convertFileSrc:()=>pixel,transformCallback:fn=>fn,invoke:async(command,args={})=>{
   const q=window.qa;q.calls.push({command,args});
   if(command==='plugin:event|listen'){q.events.set(args.event,args.handler);return 1;}
   if(command==='library_cursor')return{x:100,y:400,stickPosition:'left',displayWidth:1280,displayHeight:800};
   if(command==='library_media_info')return{previewPath:'poster.png',duration:2400,video:true};
   if(command==='video_transcript_action')return{status:'pending',text:'',error:''};
   if(command==='library_state')return args.revision===String(q.revision)?{unchanged:true}:{items:structuredClone(q.items),settings:{captureGlobal:true,tutorialCompleted:true,hoverActivation:false,language:'es',stickPosition:'left',soundEffects:false},revision:String(q.revision),version:'test'};
   return true;
  }};
  window.setProcessing=async(status)=>{
   const q=window.qa;
   q.items=[{id:'long-video',capturedAt:Date.now(),hitCount:1,pinned:false,data:{kind:'files',paths:[status==='ready'?'C:/ready.mp4':'C:/session/video-pending.mp4'],entries:[{name:'Video de Whispera',ext:'mp4',size:0,isImage:false}],...(status==='ready'?{}:{processing:{status,seconds:2400,directory:'C:/session',error:status==='error'?'Procesamiento interrumpido. Originales conservados.':undefined}})}}];
   q.revision++;q.events.get('library-changed')?.({payload:null});
  };
 });
 await page.goto(`http://127.0.0.1:${server.address().port}/browser-preview.html`);
 await page.waitForFunction(()=>window.qa.events.has('library-changed'));
 const started=Date.now();await page.evaluate(()=>window.setProcessing('processing'));
 await page.locator('[data-processing="processing"]').waitFor();
 const appearedMs=Date.now()-started;
 assert.ok(appearedMs<1000,`Processing event took ${appearedMs}ms`);
 await page.getByText('Video grabado · Procesando',{exact:true}).waitFor();
 assert.equal(await page.locator('[data-processing] .wh-video-play').count(),0);
 assert.equal(await page.locator('[data-processing] [title="Copiar"]').count(),0);
 const grid=page.locator('[data-slot="grid-reveal"] canvas');
 const first=await grid.evaluate(c=>c.toDataURL());await page.waitForTimeout(550);
 const second=await grid.evaluate(c=>c.toDataURL());assert.notEqual(first,second,'Grid Reveal must animate actual canvas pixels');
 await page.screenshot({path:resolve('../../.local/processing-video-headless.png')});
 await page.locator('[data-processing] .wh-processing-meta').click();
 assert.ok(!await page.evaluate(()=>window.qa.calls.some(c=>['copy','paste'].includes(c.args.action))),'Processing card cannot paste an incomplete MP4');
 await page.emulateMedia({reducedMotion:'reduce'});await page.waitForTimeout(250);
 const reduced=await grid.evaluate(c=>c.toDataURL());await page.waitForTimeout(250);
 assert.equal(await grid.evaluate(c=>c.toDataURL()),reduced,'Reduced motion holds still');
 await page.evaluate(()=>window.setProcessing('error'));
 await page.getByText('Video pendiente de recuperar',{exact:true}).waitFor();
 assert.equal(await page.locator('[data-slot="grid-reveal"]').count(),0,'Failure stops animating');
 await page.getByRole('button',{name:'Abrir carpeta de recuperacion',exact:true}).click();
 assert.ok(await page.evaluate(()=>window.qa.calls.some(c=>c.command==='library_action'&&c.args.action==='reveal')));
 await page.evaluate(()=>window.setProcessing('ready'));
 await page.locator('.item-main[data-id="long-video"] .wh-video-tile').waitFor();
 assert.equal(await page.locator('.item-main[data-id="long-video"]').count(),1,'Ready replaces the same card');
 assert.equal(await page.locator('[data-processing]').count(),0);
 assert.deepEqual(errors,[]);
 console.log(`PASS: processing card event-to-DOM ${appearedMs}ms (mock IPC), animated Grid Reveal pixels, reduced motion, safe actions, error and same-ID completion. Headless and silent.`);
}finally{await browser.close();await new Promise(r=>server.close(r));}
