import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});await server?.listen();
const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
const browser=await launchSilentBrowser();
async function fitNativeHost(page){
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_overlay_customize'&&c.args.action==='size'));
  await page.evaluate(()=>document.fonts.ready);
  for(let attempt=0;attempt<6;attempt++){
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const size=await page.evaluate(()=>window.calls.findLast(c=>c.command==='screen_overlay_customize'&&c.args.action==='size').args);
    const target={width:Math.ceil(size.width),height:Math.ceil(size.height)};
    if(target.width===page.viewportSize().width&&target.height===page.viewportSize().height)return;
    await page.setViewportSize(target);
  }
  assert.fail('The dock size must converge after fonts and native resizing');
}
try{
  const page=await browser.newPage({viewport:{width:640,height:480}}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(installNativeMock,{kind:'video'});
  await page.goto(base + '/overlay.html?view=screen-select');
  await page.locator('.screen-selection').waitFor();
  await page.mouse.move(100,100);await page.mouse.down();await page.mouse.move(500,300);
  assert.equal(await page.locator('.screen-selection-rect span').count(),0,'selection must not show pixel dimensions');
  await page.mouse.up();
  // The native observer grows the dock to include its new movement/size controls.
  await page.setViewportSize({width:420,height:80});
  await page.goto(base + '/overlay.html?view=screen-hud');
  await page.evaluate(()=>{window.videoStatus={phase:'recording',seconds:123,path:'',error:''};});
  const camera=page.getByRole('button',{name:'Capturar imagen del video'});
  await camera.waitFor();await fitNativeHost(page);await camera.click();
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_video_snapshot'));
  assert.deepEqual(await page.evaluate(()=>window.calls.find(c=>c.command==='screen_video_snapshot').args),{id:'test-session'});
  assert.equal(await page.evaluate(()=>window.videoStatus.phase),'recording');
  const bar=await page.locator('.capture-hud-controls').boundingBox();
  assert.ok(Math.abs(bar.x+bar.width/2-page.viewportSize().width/2)<1,'control bar centered in native host');
  const counter=await page.locator('.capture-hud-time').boundingBox();
  const indicator=await page.locator('.indicator-artwork[data-kind=camera]').boundingBox();
  assert.ok(counter.x+counter.width<=indicator.x,'camera follows the counter without covering it');
  assert.ok(indicator.x+indicator.width<=(await camera.boundingBox()).x,'camera stays clear of action buttons');
  assert.ok(indicator.y>=bar.y&&indicator.y+indicator.height<=bar.y+bar.height,'camera fits inside its bar');
  for(const name of ['Capturar imagen del video','Pausar video','Detener video','Cancelar video']){
    const box=await page.getByRole('button',{name}).boundingBox();
    assert.ok(box.x>=bar.x&&box.x+box.width<=bar.x+bar.width&&box.y>=bar.y&&box.y+box.height<=bar.y+bar.height&&box.y+box.height<=page.viewportSize().height,`${name} fits without clipping: ${JSON.stringify({box,bar})}`);
  }
  await page.getByRole('button',{name:'Pausar video'}).click();
  await page.waitForFunction(()=>window.videoStatus.phase==='paused');
  await camera.click();assert.equal(await page.evaluate(()=>window.videoStatus.phase),'paused');
  await page.screenshot({path:'../../.local/capture-controls-headless.png'});
  const image=await browser.newPage({viewport:{width:420,height:80}});
  await image.addInitScript(installNativeMock,{kind:'image',windowLabel:'image-hud'});
  // Exercise the extra native resize caused by a font arriving after first paint.
  await image.route('**/*.woff2',async route=>{await new Promise(resolve=>setTimeout(resolve,150));await route.continue();});
  await image.goto(base+'/overlay.html?view=screen-hud');
  const copy=image.getByRole('button',{name:'Copiar',exact:true});await copy.waitFor();await fitNativeHost(image);
  const styles=await copy.evaluate(button=>({width:button.clientWidth,scroll:button.scrollWidth,background:getComputedStyle(button).backgroundImage,color:getComputedStyle(button).color}));
  assert.ok(styles.width>=72&&styles.scroll<=styles.width,'Copy text fits entirely');
  assert.equal(styles.background,'none','No white gradient behind Copy');
  const actions=await image.locator('.capture-image-actions').boundingBox();
  for(const button of await image.locator('.capture-image-actions button').all()){
    const box=await button.boundingBox();assert.ok(box.x>=actions.x&&box.x+box.width<=actions.x+actions.width&&box.y>=actions.y&&box.y+box.height<=actions.y+actions.height&&box.y+box.height<=image.viewportSize().height,`Image actions fit native host: ${JSON.stringify({label:await button.getAttribute('aria-label'),box,actions,host:image.viewportSize()})}`);
  }
  await copy.click();await image.waitForFunction(()=>window.calls.some(call=>call.command==='screen_editor_action'&&call.args.action.action==='copy'));
  await image.close();
  assert.deepEqual(errors,[]);
  console.log('PASS headless: no pixel dimensions; centered video controls; camera command preserves recording/paused state; all controls fit. IPC mocked.');
}finally{await browser.close();await server?.close();}
