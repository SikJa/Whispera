import {launchSilentBrowser, silencePage} from './tests/silent-browser.mjs';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {syntheticMedia} from './tests/synthetic-media.mjs';
const fixtures=syntheticMedia();
const root=resolve('public/library');
const server=createServer(async(req,res)=>{
  try{
    if(req.url==='/fixture.mp4'){
      const bytes=await readFile(fixtures['/fixture.mp4']);
      res.setHeader('Content-Type','video/mp4');
      if(req.headers.range){const match=/bytes=(\d+)-(\d*)/.exec(req.headers.range);const start=Number(match?.[1]||0),end=Math.min(Number(match?.[2]||bytes.length-1),bytes.length-1);res.writeHead(206,{'Accept-Ranges':'bytes','Content-Range':`bytes ${start}-${end}/${bytes.length}`,'Content-Length':end-start+1});res.end(bytes.subarray(start,end+1));}else res.end(bytes);
      return;
    }
    const path=resolve(root,'.'+decodeURIComponent(new URL(req.url,'http://localhost').pathname));
    if(!path.startsWith(root+sep)){res.writeHead(403).end();return;}
    const mime={'.html':'text/html','.js':'text/javascript','.css':'text/css','.woff':'font/woff','.woff2':'font/woff2','.webm':'video/webm'};
    res.setHeader('Content-Type',mime[extname(path)]||'application/octet-stream');
    res.end(await readFile(path));
  }catch{res.writeHead(404).end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await launchSilentBrowser();
try{
  const page=await browser.newPage({viewport:{width:1280,height:800}});
  await silencePage(page);
  page.setDefaultTimeout(15000);
  const errors=[];page.on('pageerror',e=>{errors.push(e.message);console.log('PAGE ERROR',e.message);});
  await page.addInitScript(()=>{
    const pixel='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=';
    const now=Date.now();let revision=1;
    const items=[{id:'text',data:{kind:'text',text:'Prueba segura de biblioteca',isUrl:false}},
      {id:'link',data:{kind:'text',text:'https://example.org',isUrl:true}},
      {id:'image',data:{kind:'image',imageId:'C:\\fixture.png',width:1,height:1,bytes:68,ext:'png',preview:pixel}},
      {id:'video',data:{kind:'files',paths:['C:\\fixture.mp4'],entries:[{name:'fixture.mp4',ext:'mp4',size:100,isImage:false}]}},
      {id:'file',data:{kind:'files',paths:['C:\\fixture.pdf'],entries:[{name:'fixture.pdf',ext:'pdf',size:100,isImage:false}]}}]
      .map(i=>({...i,capturedAt:now,hitCount:1,pinned:false}));
    const settings={captureGlobal:true,tutorialCompleted:true,hoverActivation:false,language:'es',stickPosition:'left',soundEffects:false,...(localStorage.getItem('qa-library-background')?{backgroundColor:localStorage.getItem('qa-library-background')}:{})};
    window.__qa={calls:[],events:new Map(),settings,failSettings:false,cursor:sessionStorage.getItem('qa-library-open-tested')?null:{x:1100,y:400,stickPosition:'left',displayWidth:1280,displayHeight:800}};
    window.__TAURI_INTERNALS__={convertFileSrc:path=>/\.mp4$/i.test(path)?'/fixture.mp4':pixel,transformCallback:fn=>fn,
      invoke:async(command,args={})=>{
        window.__qa.calls.push({command,args});
        if(command==='library_cursor')return window.__qa.cursor;
        if(command==='library_drag')return new Promise((resolve,reject)=>{window.__qa.finishDrag=resolve;window.__qa.failDrag=reject;});
        if(command==='library_media_save')return new Promise((resolve,reject)=>{window.__qa.finishSave=resolve;window.__qa.failSave=reject;});
        if(command==='plugin:event|listen'){window.__qa.events.set(args.event,args.handler);return 1;}
        if(command==='library_media_info')return{previewPath:'C:\\poster.png',duration:12.7,video:true};
        if(command==='video_transcript_action')return{status:'ready',text:'Transcripcion de prueba del video.',error:''};
        if(command==='library_state')return args.revision===String(revision)?{unchanged:true}:{items:structuredClone(items),settings:{...settings},revision:String(revision),version:'0.1.6',isStoreBuild:false};
        if(command==='library_action'){
          if(args.action==='settings'&&window.__qa.failSettings)throw Error('Fallo simulado al guardar');
          if(args.action==='settings'){Object.assign(settings,args.value);if(settings.backgroundColor)localStorage.setItem('qa-library-background',settings.backgroundColor);}
          if(args.action==='pin')items.find(i=>i.id===args.id).pinned=args.value;
          if(args.action==='delete')items.splice(items.findIndex(i=>i.id===args.id),1);
          revision++;return structuredClone(settings);
        }
        return true;
      }};
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/browser-preview.html`);
  await page.waitForFunction(()=>document.querySelectorAll('.item-main').length===5,null,{timeout:15000});
  await page.waitForTimeout(600);
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_action'&&c.args.action==='settings').length),0,'Opening must never rewrite saved preferences');
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_state').length),1,'Initial hydration should reuse the first snapshot');
  await page.waitForTimeout(1200);
  assert.equal(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_window'&&c.args.operation==='hide')),false,'Keyboard-opened shelf must wait for the pointer to enter');
  await page.evaluate(()=>{window.__qa.cursor={...window.__qa.cursor,x:120};});
  await page.locator('.item-main[data-id="image"]').hover();
  const tile=page.locator('.item-main[data-id="image"]');
  const saveButton=tile.getByRole('button',{name:'Guardar imagen',exact:true});
  assert.equal(await saveButton.count(),1);
  assert.equal(await page.locator('.item-main[data-id="text"] button[aria-label="Guardar imagen"]').count(),0);
  assert.equal(await page.locator('.item-main[data-id="video"] button[aria-label="Guardar imagen"]').count(),0);
  for(const result of ['success','cancel','failure']) {
    await tile.hover();
    const before=await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_media_save').length);
    await saveButton.click();
    await page.waitForFunction(count=>window.__qa.calls.filter(c=>c.command==='library_media_save').length===count+1,before);
    assert.equal(await saveButton.isDisabled(),true);
    await page.evaluate(()=>{window.__qa.calls=window.__qa.calls.filter(c=>c.command!=='library_window');window.__qa.cursor={...window.__qa.cursor,x:1100};window.dispatchEvent(new Event('blur'));window.dispatchEvent(new Event('panel:leave'));});
    await page.waitForTimeout(650);
    assert.equal(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_window'&&c.args.operation==='hide')),false,'Save As keeps shelf open while native dialog owns focus');
    await page.evaluate(result=>{window.__qa.cursor={...window.__qa.cursor,x:120};if(result==='failure')window.__qa.failSave(Error('Destino de prueba inaccesible'));else window.__qa.finishSave(result==='cancel'?null:'C:\\Pruebas\\captura.png');},result);
    await page.waitForFunction(()=>!document.querySelector('button[aria-label="Guardar imagen"]').disabled);
    if(result==='success')await page.getByText('Imagen guardada',{exact:true}).waitFor();
    if(result==='failure')await page.getByText('No se pudo guardar la imagen. Revisá la carpeta de destino y volvé a intentar.',{exact:true}).waitFor();
  }
  assert.equal(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_action'&&['copy','paste'].includes(c.args.action))),false,'Saving must not replace clipboard or paste');
  const actionsBox=await tile.locator('.actions').boundingBox(),cardBox=await tile.boundingBox();
  assert.ok(actionsBox.x>=cardBox.x&&actionsBox.x+actionsBox.width<=cardBox.x+cardBox.width,'Quick actions fit inside the image tile');
  await tile.screenshot({path:'../../.local/save-image-shortcut.png'});
  await page.getByText('No se pudo guardar la imagen. Revisá la carpeta de destino y volvé a intentar.',{exact:true}).waitFor({state:'hidden'});
  assert.equal(await tile.getAttribute('draggable'),'false','Tauri files must not enter a second HTML5 OS drag');
  const tileBox=await tile.boundingBox();
  const dragX=tileBox.x+10,dragY=tileBox.y+tileBox.height*.55;
  await page.mouse.move(dragX,dragY);await page.mouse.down();await page.mouse.move(dragX+25,dragY);
  await page.waitForFunction(()=>window.__qa.calls.some(c=>c.command==='library_drag'&&c.args.id==='image'));
  await page.mouse.move(dragX+40,dragY);await page.mouse.up();
  assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_drag').length),1,'one native drag per pointer gesture');
  await page.evaluate(()=>window.__qa.finishDrag(true));await page.waitForTimeout(250);
  for(const id of ['image','video']){
    await page.evaluate(id=>{
      window.__qa.calls.length=0;
      window.edge.startDrag({id});
      window.__qa.cursor={...window.__qa.cursor,x:1100};
      document.querySelector(`[data-id="${id}"]`).dispatchEvent(new DragEvent('dragend',{bubbles:true}));
      window.dispatchEvent(new Event('blur'));
      window.dispatchEvent(new Event('panel:leave'));
    },id);
    await page.waitForTimeout(1900);
    assert.equal(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_window'&&c.args.operation==='hide')),false,'Blur/DOM dragend/cursor exit must not cancel a native drag');
    await page.evaluate(()=>{window.__qa.cursor={...window.__qa.cursor,x:120};window.__qa.finishDrag(true);});
    await page.waitForTimeout(250);
  }
  await page.evaluate(()=>{
    window.__qa.calls.length=0;
    window.__qa.cursor={...window.__qa.cursor,x:1100};
    window.__qa.events.get('library-open')({});
  });
  await page.waitForTimeout(900);
  assert.equal(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_window'&&c.args.operation==='hide')),false,'Explicit reopen cancels stale outside-cursor dismissal');
  await page.evaluate(()=>{window.__qa.cursor={...window.__qa.cursor,x:120};});
  await page.waitForTimeout(150);
  await page.evaluate(()=>{window.__qa.cursor=null;sessionStorage.setItem('qa-library-open-tested','true');});
  assert.equal(await page.locator('#preview-controls').count(),0);
  const filters=await page.locator('.header button').evaluateAll(el=>el.map(b=>b.title||b.getAttribute('aria-label')||b.textContent));
  console.log('Filter buttons:',JSON.stringify(filters));
  assert.ok(!filters.some(t=>/emoji|colores|colors/i.test(t)));
  assert.ok(filters.includes('Videos'));
  await page.locator('.header button[title="Videos"]').click();
  await page.waitForFunction(()=>document.querySelectorAll('.item-main').length===1,null,{timeout:15000});
  assert.equal(await page.locator('.item-main').getAttribute('data-id'),'video');
  await page.locator('.wh-video-time').waitFor();
  await page.waitForFunction(()=>document.querySelector('.wh-video-time')?.textContent==='0:12');
  assert.equal(await page.locator('.wh-video-time').textContent(),'0:12');
  await page.waitForFunction(()=>document.querySelector('.wh-video-tile img')?.naturalWidth>0);
  assert.ok(await page.locator('.wh-video-tile img').evaluate(image=>image.naturalWidth>0));
  await page.getByText('Voz transcrita',{exact:true}).waitFor();
  await page.locator('.item-main').hover();
  await page.locator('.item-main .preview-expand').focus();
  await page.keyboard.press('Enter');
  await page.getByLabel('Texto del video').waitFor();
  assert.equal(await page.locator('video[controls]').count(),0);
  await page.waitForFunction(()=>document.querySelector('.library-video-player video').readyState>=2);
  await page.getByRole('button',{name:'Reproducir video',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.library-video-player video').currentTime>0);
  assert.ok(await page.locator('.library-video-player video').evaluate(v=>v.muted&&v.volume===0));
  await page.getByRole('button',{name:'Pausar video',exact:true}).click();
  await page.getByRole('slider',{name:'Posicion del video'}).fill('1');
  assert.ok(await page.locator('.library-video-player video').evaluate(v=>Math.abs(v.currentTime-1)<.2));
  const mediaBox=await page.locator('.library-video-player video').boundingBox();
  const controlsBox=await page.locator('.library-video-controls').boundingBox();
  assert.ok(controlsBox.y>=mediaBox.y+mediaBox.height-1,'Video controls must not cover the image');
  assert.equal(await page.locator('[title="Acceso rapido"]').count(),0);
  assert.equal(await page.locator('[title="Editar imagen"]').count(),0,'No image editor on videos');
  assert.equal(await page.getByLabel('Texto del video').inputValue(),'Transcripcion de prueba del video.');
  await page.getByRole('button',{name:'Copiar texto',exact:true}).click();
  assert.ok(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='video_transcript_action'&&c.args.action==='copy')));
  await page.locator('.item-main .preview-contract').focus();
  await page.keyboard.press('Enter');
  await page.locator('.header button[title="Archivos"]').click();
  await page.waitForFunction(()=>document.querySelectorAll('.item-main').length===1,null,{timeout:15000});
  assert.equal(await page.locator('.item-main').getAttribute('data-id'),'file');
  await page.locator('.header button[title="Todos"]').click();
  await page.waitForFunction(()=>document.querySelectorAll('.item-main').length===5,null,{timeout:15000});
  await page.locator('.item-main[data-id="image"]').hover();
  for(const id of ['text','link','file'])assert.equal(await page.locator(`.item-main[data-id="${id}"] [title="Editar imagen"]`).count(),0);
  await page.locator('.item-main[data-id="image"]').getByTitle('Editar imagen',{exact:true}).click();
  assert.ok(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_media_open'&&c.args.mode==='canvas'&&c.args.id==='image')));
  await page.evaluate(async()=>{await window.edge.copySubitem({id:'video',paths:['C:\\fixture.mp4']});window.edge.startDrag({id:'video',paths:['C:\\fixture.mp4']});});
  await page.waitForFunction(()=>window.__qa.calls.some(c=>c.command==='library_drag'));
  await page.evaluate(()=>window.__qa.finishDrag(true));
  assert.ok(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='library_action'&&c.args.action==='copy'&&c.args.value.paths[0]==='C:\\fixture.mp4')));
  await page.evaluate(()=>window.edge.setPinned('text',true));
  assert.ok((await page.evaluate(()=>window.edge.loadState())).items.find(i=>i.id==='text').pinned);
  const region=await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='recorder_region').at(-1));
  assert.ok(region.args.rects.some(r=>r.width<400));
  await page.screenshot({path:'../../.local/library-native-headless.png',omitBackground:true});
  assert.equal(await page.locator('.footer-capsule-count').count(),0);
  await page.locator('.header button[title="Configuración"]').click();
  await page.waitForTimeout(500);
  const settingsText=await page.locator('.blade').innerText();
  assert.ok(!/COMUNIDAD|COMMUNITY|Edge-Drop v/i.test(settingsText));
  assert.ok(!settingsText.includes('Acceso rapido de capturas'));
  await page.getByRole('button',{name:/^(Buscar actualizaciones|Abrir actualizaciones)$/}).click();
  await page.waitForTimeout(150);
  assert.ok(await page.evaluate(()=>window.__qa.calls.some(c=>c.command==='open_updates')),'Shelf updates must open the real Whispera updater');
  assert.equal(await page.getByRole('button',{name:'Notificarme',exact:true}).count(),0,'No disconnected duplicate update preferences');
  await page.getByRole('button',{name:'Apariencia',exact:true}).click();
  await page.locator('.library-background-card .palette-trigger').click();
  await page.getByRole('button',{name:'Morado',exact:true}).click();
  await page.waitForFunction(()=>window.__qa.settings.backgroundColor==='#9024DC');
  assert.equal(await page.locator('.blade').evaluate(el=>getComputedStyle(el).backgroundColor),'rgb(144, 36, 220)');
  await page.waitForFunction(()=>window.__qa.calls.filter(c=>c.command==='recorder_region').at(-1)?.args.rects.some(r=>r.width>=300&&r.height>200));
  await page.getByRole('button',{name:'Blanco',exact:true}).click();
  await page.waitForFunction(()=>window.__qa.settings.backgroundColor==='#F3F3F3');
  assert.equal(await page.locator('.setting-title').first().evaluate(el=>getComputedStyle(el).color),'rgb(23, 23, 23)','white background needs dark chrome/text');
  await page.waitForFunction(()=>getComputedStyle(document.querySelector('.setting-card')).backgroundColor==='rgb(225, 225, 225)');
  await page.waitForFunction(()=>getComputedStyle(document.querySelector('.header .icon-btn.active')).color==='rgb(23, 23, 23)');
  await page.screenshot({path:'../../.local/library-background-white.png',omitBackground:true});
  await page.getByRole('button',{name:'Cerrar paleta',exact:true}).click();
  await page.locator('.header .icon-btn.active').click();
  await page.locator('.item-main[data-id="image"]').hover();
  const ink=await page.locator('.item-main[data-id="image"] .actions .act').first().evaluate(el=>getComputedStyle(el).color);
  assert.equal(ink,'rgb(244, 244, 244)','dark action pill retains white icons on a white shelf');
  await page.locator('.header button[title="Configuración"]').click();
  await page.getByRole('button',{name:'Apariencia',exact:true}).click();
  await page.locator('.library-background-card .palette-trigger').click();
  await page.evaluate(()=>window.dispatchEvent(new Event('blur')));await page.waitForTimeout(600);
  assert.equal(await page.locator('.library-background-card').isVisible(),true,'appearance settings must not close while choosing a color');
  await page.getByRole('button',{name:'Personalizado',exact:true}).click();
  await page.getByRole('textbox',{name:'Código HEX'}).fill('#12AB67');
  await page.getByRole('textbox',{name:'Código HEX'}).press('Enter');
  await page.waitForFunction(()=>window.__qa.settings.backgroundColor?.toLowerCase()==='#12ab67');
  assert.equal(await page.evaluate(()=>window.__qa.settings.soundEffects),false,'color changes preserve unrelated preferences');
  await page.getByRole('button',{name:'Cerrar paleta',exact:true}).click();
  await page.reload();
  await page.waitForFunction(()=>document.documentElement.dataset.libraryColor?.toLowerCase()==='#12ab67');
  await page.locator('.header button[title="Configuración"]').click();
  await page.getByRole('button',{name:'Posición',exact:true}).click();
  await page.locator('.wh-edge-tabs').waitFor();
  assert.ok(await page.locator('.wh-edge-tabs button span').evaluateAll(labels=>labels.every(el=>el.scrollWidth<=el.clientWidth+1)));
  await page.getByRole('button',{name:'Borde derecho',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.blade-right')&&!document.querySelector('.wh-edge-tabs button:disabled'));
  await page.getByRole('button',{name:'Borde superior',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.blade-top')&&!document.querySelector('.wh-edge-tabs button:disabled'));
  await page.evaluate(()=>window.edge.updateSettings({horizontalOffset:0}));
  const left=await page.locator('.blade').evaluate(el=>el.getBoundingClientRect().x);
  await page.evaluate(()=>window.edge.updateSettings({horizontalOffset:1}));
  await page.waitForFunction(left=>document.querySelector('.blade').getBoundingClientRect().x>left+50,left);
  await page.getByRole('button',{name:'Borde izquierdo',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.blade-left')&&!document.querySelector('.wh-edge-tabs button:disabled'));
  await page.screenshot({path:'../../.local/library-position-headless.png',omitBackground:true});
  await page.evaluate(()=>window.__qa.failSettings=true);
  await page.getByRole('button',{name:'Borde derecho',exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Fallo simulado'}).waitFor();
  assert.equal(await page.locator('.wh-edge-tabs button:disabled').count(),0);
  assert.equal(await page.locator('.blade-container').evaluate(el=>getComputedStyle(el).pointerEvents),'auto');
  await page.evaluate(()=>window.__qa.failSettings=false);
  await page.emulateMedia({reducedMotion:'reduce'});
  await page.getByRole('button',{name:'Borde derecho',exact:true}).click();
  await page.waitForFunction(()=>Boolean(document.querySelector('.blade-right')));
  assert.equal(await page.locator('.wh-edge-pill').evaluate(el=>getComputedStyle(el).transitionDuration),'0s');
  await page.getByRole('button',{name:'Borde izquierdo',exact:true}).click();
  await page.waitForFunction(()=>Boolean(document.querySelector('.blade-left')));
  await page.getByRole('button',{name:'Apariencia',exact:true}).click();
  await page.locator('.library-background-card .palette-trigger').click();
  await page.getByRole('button',{name:'Negro',exact:true}).click();
  await page.waitForFunction(()=>window.__qa.settings.backgroundColor==='#000000'&&!document.documentElement.dataset.libraryColor);
  assert.equal(await page.locator('.blade').evaluate(el=>getComputedStyle(el).backgroundColor),'rgb(0, 0, 0)');
  assert.equal(await page.evaluate(()=>document.documentElement.style.getPropertyValue('--bg-card')),'','black restores the original theme');
  await page.getByRole('button',{name:'Cerrar paleta',exact:true}).click();
  for(const close of ['escape','native']) {
    if(close==='escape')await page.keyboard.press('Escape');
    else await page.evaluate(()=>window.__qa.events.get('library-close')({}));
    await page.waitForFunction(()=>window.__qa.calls.filter(c=>c.command==='library_window').at(-1)?.args.operation==='hide');
    const shows=await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_window'&&c.args.operation==='show').length);
    await page.waitForTimeout(650);
    assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.command==='library_window'&&c.args.operation==='show').length),shows,'closed panel must not reopen from DOM polling');
    await page.evaluate(()=>window.__qa.events.get('library-open')({}));
    await page.waitForFunction(shows=>window.__qa.calls.filter(c=>c.command==='library_window'&&c.args.operation==='show').length>shows,shows);
  }
  assert.deepEqual(errors,[]);
  console.log('PASS native adapter: hydration, filters, no demo controls, subitem copy, file drag IPC, pin, clipped geometry; background presets/custom, persistence, contrast and original black restoration. OS clipboard/drag mocked; no visible Windows tests.');
}finally{await browser.close();await new Promise(r=>server.close(r));}
