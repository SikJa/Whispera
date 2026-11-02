import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
const base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';
const errors=[];
async function selector({saved='none',kind='video',delayed=false,failStart=false,width=900}={}) {
  const page=await browser.newPage({viewport:{width,height:650}});
  page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(installNativeMock,{kind});
  await page.addInitScript(({saved,delayed,failStart})=>{
    window.videoPreferences.audio=saved;
    const original=window.__TAURI_INTERNALS__.invoke;let firstLoad=delayed,firstStart=failStart;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='screen_preferences'&&firstLoad){firstLoad=false;return new Promise((resolve,reject)=>{window.finishAudioLoad=()=>resolve({...window.videoPreferences});window.failAudioLoad=()=>reject(Error('No se pudieron leer las preferencias de prueba'));});}
      if(command==='screen_start'&&firstStart){firstStart=false;throw Error('No se pudo iniciar el video de prueba');}
      return original(command,args);
    };
  },{saved,delayed,failStart});
  await page.goto(base+'/overlay.html?view=screen-select');
  await page.locator('.screen-selection').waitFor();
  return page;
}
try {
  for(const [audio,label] of [['none','Sin audio'],['microphone','Micrófono'],['system','Computadora'],['both','Ambos']]) {
    const page=await selector({saved:'system'});
    await page.waitForFunction(()=>!document.querySelector('.screen-selection-audio button')?.disabled);
    assert.equal(await page.getByRole('button',{name:'Computadora',exact:true}).getAttribute('aria-pressed'),'true');
    await page.getByRole('button',{name:label,exact:true}).click();
    assert.equal(await page.evaluate(()=>window.calls.filter(call=>call.command==='screen_start').length),0,'changing audio must not start a recording');
    assert.equal(await page.evaluate(()=>window.videoPreferences.audio),'system','per-video choice must preserve the saved default');
    await page.keyboard.press('Control+a');
    await page.waitForFunction(()=>window.calls.some(call=>call.command==='screen_start'));
    assert.deepEqual(await page.evaluate(()=>window.calls.find(call=>call.command==='screen_start').args),{rect:{x:0,y:0,width:900,height:650},audio});
    assert.equal(await page.evaluate(()=>window.calls.filter(call=>call.command==='screen_save_preferences').length),0);
    await page.close();
  }
  const delayed=await selector({saved:'both',delayed:true});
  await delayed.waitForFunction(()=>!!window.finishAudioLoad);
  assert.equal(await delayed.getByRole('button',{name:'Ambos',exact:true}).isDisabled(),true);
  await delayed.keyboard.press('Control+a');
  assert.equal(await delayed.evaluate(()=>window.calls.some(call=>call.command==='screen_start')),false,'must not fall back to silent capture while preferences are loading');
  await delayed.evaluate(()=>window.failAudioLoad());
  await delayed.getByRole('alert').waitFor();
  await delayed.keyboard.press('Control+a');
  assert.equal(await delayed.evaluate(()=>window.calls.some(call=>call.command==='screen_start')),false,'failed preference loading blocks capture');
  await delayed.getByRole('button',{name:'Reintentar',exact:true}).click();
  await delayed.waitForFunction(()=>!document.querySelector('.screen-selection-audio button')?.disabled);
  await delayed.keyboard.press('Control+a');
  await delayed.waitForFunction(()=>window.calls.some(call=>call.command==='screen_start'&&call.args.audio==='both'));
  await delayed.close();

  const retry=await selector({saved:'system',failStart:true});
  await retry.getByRole('button',{name:'Ambos',exact:true}).click();
  await retry.keyboard.press('Control+a');
  await retry.getByText(/No se pudo iniciar el video de prueba/).waitFor();
  await retry.keyboard.press('Control+a');
  await retry.waitForFunction(()=>window.calls.some(call=>call.command==='screen_start'&&call.args.audio==='both'));
  await retry.close();

  const narrow=await selector({width:360});
  await narrow.getByText('Este video se grabará sin sonido.').waitFor();
  const bounds=await narrow.locator('.screen-selection-help').boundingBox();
  assert.ok(bounds.x>=0&&bounds.x+bounds.width<=360&&bounds.y+bounds.height<=650,'audio controls fit the viewport');
  for(const button of await narrow.locator('.screen-selection-audio button').all()) {
    const box=await button.boundingBox();assert.ok(box.x>=bounds.x&&box.x+box.width<=bounds.x+bounds.width,'audio choices fit without horizontal clipping');
  }
  await narrow.screenshot({path:'../../.local/capture-audio-preview.png'});
  await narrow.close();

  const image=await selector({kind:'image'});
  assert.equal(await image.getByRole('group',{name:'Sonido del video'}).count(),0);
  await image.keyboard.press('Control+a');
  await image.waitForFunction(()=>window.calls.some(call=>call.command==='screen_start'));
  assert.equal(await image.evaluate(()=>Object.hasOwn(window.calls.find(call=>call.command==='screen_start').args,'audio')),false,'screenshots do not request audio');
  await image.close();
  assert.deepEqual(errors,[]);
  console.log('PASS: four session audio modes; inherited defaults; no accidental capture on audio clicks; delayed/failed preferences and retry; capture retry; narrow controls; images unchanged. Native IPC mocked, no screen or microphone captured.');
} finally {await browser.close();}
