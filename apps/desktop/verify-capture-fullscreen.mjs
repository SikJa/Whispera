import assert from 'node:assert/strict';
import {mkdirSync} from 'node:fs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';
const browser=await launchSilentBrowser();
mkdirSync('../../.local/capture-preview',{recursive:true});
try {
 for(const kind of ['image','video']){
  const page=await browser.newPage({viewport:{width:1366,height:900}}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto(`${base}/capture-preview.html?kind=${kind}`);
  const tools=page.frameLocator('iframe[title="Herramientas"]');
  const hud=page.frameLocator('iframe[title="Acciones"]');
  await tools.getByRole('button',{name:'Contraer herramientas',exact:true}).waitFor();
  await tools.getByRole('button',{name:'Orientar herramientas horizontalmente'}).click();
  await tools.locator('[data-horizontal="true"]').waitFor();
  await tools.getByRole('button',{name:'Orientar herramientas verticalmente'}).click();
  for(let i=0;i<3;i++){
   await tools.getByRole('button',{name:'Contraer herramientas',exact:true}).click();
   await tools.getByRole('button',{name:'Expandir herramientas',exact:true}).click();
  }
  await tools.getByRole('button',{name:'Reducir herramientas',exact:true}).click();
  await tools.getByRole('button',{name:'Reducir herramientas',exact:true}).click();
  await tools.getByRole('button',{name:'Contraer herramientas',exact:true}).click();
  const label=kind==='video'?'controles de video':'acciones de captura';
  await hud.getByRole('button',{name:`Reducir ${label}`,exact:true}).click();
  await hud.getByRole('button',{name:`Contraer ${label}`,exact:true}).click();
  await page.waitForTimeout(500);
  const sizes=()=>page.locator('iframe').evaluateAll(frames=>frames.map(f=>({width:f.offsetWidth,height:f.offsetHeight,left:f.offsetLeft,top:f.offsetTop})));
  const before=await sizes();await page.waitForTimeout(1800);assert.deepEqual(await sizes(),before,'Native-size feedback must settle after collapse');
  for(const frame of page.frames().filter(f=>f!==page.mainFrame())){
   const count=await frame.evaluate(()=>window.calls.filter(c=>c.command==='screen_overlay_customize'&&c.args.action==='size').length);
   assert.ok(count<40,`Unbounded resizing: ${count}`);
  }
  await page.screenshot({path:`../../.local/capture-preview/${kind}-compact.png`});
  await tools.getByRole('button',{name:'Expandir herramientas',exact:true}).click();
  await hud.getByRole('button',{name:`Expandir ${label}`,exact:true}).click();
  await tools.getByRole('button',{name:'Orientar herramientas horizontalmente'}).click();
  await page.waitForTimeout(500);
  for(const width of [1920,1366,1024]){
   await page.setViewportSize({width,height:900});await page.waitForTimeout(150);
   for(const b of await sizes())assert.ok(b.left>=0&&b.top>=0&&b.left+b.width<=width&&b.top+b.height<=836);
  }
  await page.screenshot({path:`../../.local/capture-preview/${kind}-horizontal.png`});
  assert.deepEqual(errors,[]);await page.close();
 }
 const selector=await browser.newPage();
 await selector.addInitScript(installNativeMock,{kind:'video'});
 await selector.goto(`${base}/overlay.html?view=screen-select`);
 await selector.getByRole('button',{name:/Pantalla 2/}).click();
 assert.ok(await selector.evaluate(()=>window.calls.some(c=>c.command==='screen_start_monitor'&&c.args.index===1)));
 await selector.close();
 console.log('PASS: monitor selection; full-screen image/video docks; orientation; repeated collapse; settled native-size feedback at zoom < 1; screenshots. Native IPC simulated, no screen recording.');
}finally{await browser.close();}
