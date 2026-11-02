import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage();const modules=new Set();
  page.on('request',r=>{const path=new URL(r.url()).pathname;if(path.startsWith('/src/'))modules.add(path);});
  await page.addInitScript(installNativeMock,{kind:'image',selectionPhase:'selecting'});
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-select');
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_overlay_ready'));
  console.log(JSON.stringify({selectorSourceModules:modules.size,loadsEditor:modules.has('/src/ScreenEditor.tsx'),loadsReplaySettings:modules.has('/src/ReplaySettings.tsx')}));
  assert.ok(!modules.has('/src/ScreenEditor.tsx'),'selector must not load the independent annotation editor');
  assert.ok(!modules.has('/src/ReplaySettings.tsx'),'selector must not load replay settings');
  assert.equal(await page.locator('.screen-selection').count(),1);
  await page.close();
  const ink=await browser.newPage();await ink.addInitScript(installNativeMock,{kind:'image'});
  await ink.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-ink');
  await ink.waitForFunction(()=>window.feedback.busy===false);
  assert.equal(await ink.getByLabel('Editar captura',{exact:true}).count(),1);
  console.log('PASS: capture entry loads only its view; selector and ink retain readiness. Development module graph, IPC mocked.');
} finally {await browser.close();}
