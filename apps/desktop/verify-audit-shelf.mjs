import {launchSilentBrowser, silencePage} from './tests/silent-browser.mjs';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,writeFile} from 'node:fs/promises';
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
    const settings={captureGlobal:true,tutorialCompleted:true,hoverActivation:false,language:'es',stickPosition:'left',soundEffects:false};
    window.__qa={calls:[],events:new Map(),settings,failSettings:false};
    window.__TAURI_INTERNALS__={convertFileSrc:path=>/\.mp4$/i.test(path)?'/fixture.mp4':pixel,transformCallback:fn=>fn,
      invoke:async(command,args={})=>{
        window.__qa.calls.push({command,args});
        if(command==='plugin:event|listen'){window.__qa.events.set(args.event,args.handler);return 1;}
        if(command==='library_media_info')return{previewPath:'C:\\poster.png',duration:12.7,video:true};
        if(command==='video_transcript_action')return{status:'ready',text:'Transcripcion de prueba del video.',error:''};
        if(command==='library_state')return args.revision===String(revision)?{unchanged:true}:{items:structuredClone(items),settings:{...settings},revision:String(revision),version:'0.1.6',isStoreBuild:false};
        if(command==='library_action'){
          if(args.action==='settings'&&window.__qa.failSettings)throw Error('Fallo simulado al guardar');
          if(args.action==='settings')Object.assign(settings,args.value);
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
  assert.equal(await page.locator('.wh-video-time').innerText(),'0:12');
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


  const cases=[];
  const card=title=>page.locator('.setting-card').filter({has:page.locator('.setting-title').getByText(title,{exact:true})});
  const setPill=async(title,name,key,value)=>{
    await card(title).getByRole('button',{name,exact:true}).click();
    await page.waitForFunction(({key,value})=>window.__qa.settings[key]===value,{key,value});
    cases.push({title,name,key,value,passed:true});
  };
  const toggle=async(title,key)=>{
    const el=card(title).getByRole('switch');
    for(let i=0;i<2;i++){
      const value=await el.getAttribute('aria-checked')!=='true';await el.click();
      await page.waitForFunction(({key,value})=>window.__qa.settings[key]===value,{key,value});
      assert.equal(await el.getAttribute('aria-checked'),String(value));cases.push({title,key,value,passed:true});
    }
  };
  await page.getByRole('button',{name:'Comportamiento',exact:true}).click();
  for(const [title,key] of [['Iniciar al iniciar sesión','launchAtLogin'],['Modo incógnito','incognito'],['Mover los elementos pegados arriba','movePastedToTop'],['Borrar no fijados al reiniciar','clearUnpinnedOnRestart']])await toggle(title,key);
  await card('Activación al pasar el cursor').getByRole('switch').click();
  await page.waitForFunction(()=>window.__qa.settings.hoverActivation===true);
  await toggle('Protección de pantalla completa','suppressInFullscreen');
  await card('Activación al pasar el cursor').getByRole('switch').click();
  await page.waitForFunction(()=>window.__qa.settings.hoverActivation===false);
  assert.equal(await card('Protección de pantalla completa').getByRole('switch').isDisabled(),true);
  cases.push({title:'Hover → fullscreen dependency',passed:true});
  for(const n of [100,250,500,1000])await setPill('Capacidad del historial',String(n),'historyLimit',n);
  for(const [name,value] of [['Nunca',0],['1h',1],['6h',6],['24h',24],['7d',168]])await setPill('Temporizador de eliminación automática',name,'autoDeleteHours',value);
  await page.evaluate(()=>window.__qa.failSettings=true);
  const launch=card('Iniciar al iniciar sesión').getByRole('switch');const old=await launch.getAttribute('aria-checked');await launch.click();
  await page.getByRole('alert').filter({hasText:'Fallo simulado al guardar'}).waitFor();
  assert.equal(await launch.getAttribute('aria-checked'),old,'Failed startup write must not pretend to be saved');
  await page.evaluate(()=>window.__qa.failSettings=false);
  await toggle('Iniciar al iniciar sesión','launchAtLogin');
  cases.push({title:'Settings save failure, unchanged value, retry',passed:true});
  await page.getByRole('button',{name:'Posición',exact:true}).click();
  await toggle('Indicador de posición en el borde','showEdgeLocationHint');
  for(const [name,value] of [['Arriba','top'],['Centro','center'],['Abajo','bottom']])await setPill('Posición del activador de borde',name,'triggerAlignment',value);
  for(const [name,value] of [['0%',0],['Center',.5],['100%',1]])await setPill('Posición vertical',name,'verticalOffset',value);
  for(const title of ['Tamaño del área de desplazamiento','Grosor del activador de borde','Altura del panel']){
    for(const el of await card(title).locator('button:not([role="slider"])').all()){
      const name=await el.innerText();const n=await page.evaluate(()=>window.__qa.calls.length);await el.click();
      await page.waitForFunction(n=>window.__qa.calls.slice(n).some(c=>c.command==='library_action'&&c.args.action==='settings'),n);
      cases.push({title,name,passed:true,effect:'Settings IPC patch; native geometry separately limited'});
    }
  }
  await page.getByRole('button',{name:'Apariencia',exact:true}).click();
  await toggle('Indicador de copia','showCopyIndicator');
  for(const [name,value] of [['Pequeño',.85],['Normal',1],['Grande',1.15]])await setPill('Tamaño del texto',name,'fontSizeScale',value);
  await toggle('Efectos de sonido','soundEffects');
  await page.locator('.style-preview-toggle-btn').click();
  await page.locator('.indicator-card').first().waitFor();
  for(const [index,value] of ['logo','check','copy','sparkle'].entries()){
    await page.locator('.indicator-card').nth(index).click();
    await page.waitForFunction(value=>window.__qa.settings.copyIndicatorStyle===value,value);
    assert.ok(await page.locator('.indicator-card').nth(index).evaluate(el=>el.classList.contains('active')));
    cases.push({title:'Estilo indicador',value,passed:true});
  }
  await page.locator('.style-preview-toggle-btn').click();
  await page.getByRole('button',{name:'Comportamiento',exact:true}).click();
  await page.locator('.hotkey-field').click();
  await page.keyboard.press('Control+Alt+F9');
  await page.waitForFunction(()=>window.__qa.settings.toggleHotkey==='Ctrl+Alt+F9');
  await page.locator('.hotkey-field').click();await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(()=>window.__qa.settings.toggleHotkey),'Ctrl+Alt+F9');
  await page.locator('.hotkey-inline-reset-btn').click();
  await page.waitForFunction(()=>window.__qa.settings.toggleHotkey==='Alt+C');
  cases.push({title:'Atajo: capturar combinación, cancelar Escape, restaurar Alt+C',passed:true});
  const langCard=page.locator('.setting-card').filter({has:page.locator('.shelf-card-bottom button[data-active]')});
  await page.locator('.setting-card').filter({has:page.locator('.shelf-card-bottom')}).first().locator('.shelf-card-bottom button').first().click();
  const languages=await langCard.locator('button[data-active]').allTextContents();
  for(let i=0;i<languages.length;i++){
    const target=langCard.locator('button[data-active]').nth(i);
    const label=await target.innerText();await target.click();
    await page.waitForTimeout(100);
    const value=await page.evaluate(()=>window.__qa.settings.language);
    cases.push({title:'Idioma del estante',label,value,passed:true,effect:'Seleccionado, persistido en adaptador simulado; no traducción semántica auditada'});
    // The card stays first below the promoted updater; its text changes with locale.
    await page.locator('.setting-card').filter({has:page.locator('.shelf-card-bottom')}).first().locator('.shelf-card-bottom button').first().click();
    assert.equal(await langCard.locator('button[data-active="true"]').count(),1);
  }
  const spanish=langCard.locator('button[data-active]').filter({hasText:'Español'}).first();await spanish.click();
  await page.getByRole('button',{name:'Apariencia',exact:true}).waitFor();
  await page.evaluate(()=>window.__qa.opened=[]);
  await page.evaluate(()=>window.open=url=>{window.__qa.opened.push(url);return null;});
  await page.getByRole('button',{name:'Novedades',exact:true}).click();
  assert.ok((await page.evaluate(()=>window.__qa.opened)).some(url=>url==='https://github.com/kazu00001/Whispera-K/releases'));
  cases.push({title:'Novedades uses correct release repository; window.open intercepted',passed:true});
  await writeFile('../../outputs/auditoria-2026-10-06/evidencias/shelf-options.json',JSON.stringify(cases,null,2));

  const controlInventory=[];
  for(const section of ['Comportamiento','Posición','Apariencia']){
    await page.getByRole('button',{name:section,exact:true}).click();await page.waitForTimeout(250);
    const controls=await page.locator('.blade button,.blade input,.blade select,[role="menuitem"]').evaluateAll(elements=>elements.filter(e=>e.getClientRects().length&&getComputedStyle(e).visibility!=='hidden').map(e=>({tag:e.tagName,type:e.type||'',role:e.getAttribute('role')||'',label:e.getAttribute('aria-label')||e.title||e.textContent?.trim().slice(0,180)||'',context:e.closest('.setting-card,.settings-shelf-card')?.querySelector('.setting-title')?.textContent||'',disabled:e.disabled||false,value:e.type==='password'?'redacted':e.value,options:e.tagName==='SELECT'?[...e.options].map(o=>({value:o.value,label:o.textContent})):[],min:e.min,max:e.max})));
    controlInventory.push({section,controls});
  }
  await writeFile('../../outputs/auditoria-2026-10-06/estante-controles-visibles.json',JSON.stringify(controlInventory,null,2));
  console.log('SHELF INVENTORY',JSON.stringify(controlInventory));
  assert.deepEqual(errors,[]);
  console.log('PASS native adapter: hydration, filters, no demo controls, subitem copy, file drag IPC, pin, clipped geometry. OS clipboard/drag mocked; no visible Windows tests.');
}finally{await browser.close();await new Promise(r=>server.close(r));}
