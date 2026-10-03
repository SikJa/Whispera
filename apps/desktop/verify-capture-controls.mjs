import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try{
  const page=await browser.newPage({viewport:{width:640,height:480}}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(installNativeMock,{kind:'video'});
  await page.goto((process.env.WHISPERA_TEST_URL || 'http://127.0.0.1:5190') + '/overlay.html?view=screen-select');
  await page.locator('.screen-selection').waitFor();
  await page.mouse.move(100,100);await page.mouse.down();await page.mouse.move(500,300);
  assert.equal(await page.locator('.screen-selection-rect span').count(),0,'selection must not show pixel dimensions');
  await page.mouse.up();
  await page.setViewportSize({width:260,height:56});
  await page.goto((process.env.WHISPERA_TEST_URL || 'http://127.0.0.1:5190') + '/overlay.html?view=screen-hud');
  await page.evaluate(()=>{window.videoStatus={phase:'recording',seconds:123,path:'',error:''};});
  const camera=page.getByRole('button',{name:'Capturar imagen del video'});
  await camera.waitFor();await camera.click();
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_video_snapshot'));
  assert.deepEqual(await page.evaluate(()=>window.calls.find(c=>c.command==='screen_video_snapshot').args),{id:'test-session'});
  assert.equal(await page.evaluate(()=>window.videoStatus.phase),'recording');
  const bar=await page.locator('.capture-hud-controls').boundingBox();
  assert.ok(Math.abs(bar.x+bar.width/2-130)<1,'control bar centered in native host');
  for(const name of ['Capturar imagen del video','Pausar video','Detener video','Cancelar video']){
    const box=await page.getByRole('button',{name}).boundingBox();
    assert.ok(box.x>=bar.x&&box.x+box.width<=bar.x+bar.width&&box.y>=0&&box.y+box.height<=56,`${name} fits without clipping`);
  }
  await page.getByRole('button',{name:'Pausar video'}).click();
  await page.waitForFunction(()=>window.videoStatus.phase==='paused');
  await camera.click();assert.equal(await page.evaluate(()=>window.videoStatus.phase),'paused');
  await page.screenshot({path:'../../.local/capture-controls-headless.png'});
  assert.deepEqual(errors,[]);
  console.log('PASS headless: no pixel dimensions; centered video controls; camera command preserves recording/paused state; all controls fit. IPC mocked.');
}finally{await browser.close();}
