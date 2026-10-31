import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage();
  await page.addInitScript(installNativeMock,{kind:'image'});
  await page.addInitScript(()=>{
    window.testHidden=true;
    Object.defineProperty(document,'hidden',{get:()=>window.testHidden});
  });
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-hud');
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready'));
  const count=()=>page.evaluate(()=>window.calls.filter(c=>c.command==='ui_heartbeat').length);
  const before=await count();
  await page.waitForTimeout(2200);
  assert.equal(await count(),before,'hidden surfaces must not schedule recurring heartbeat IPC');
  await page.evaluate(()=>{window.testHidden=false;document.dispatchEvent(new Event('visibilitychange'));});
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='ui_heartbeat'));
  assert.equal(await count(),before+1,'showing a surface immediately restores its heartbeat');
  await page.waitForTimeout(2200);
  assert.equal(await count(),before+2,'visible surfaces keep the watchdog heartbeat');
  await page.evaluate(()=>{window.testHidden=true;document.dispatchEvent(new Event('visibilitychange'));});
  await page.waitForTimeout(2200);
  assert.equal(await count(),before+2,'hiding cancels the scheduled heartbeat');
  console.log('PASS: hidden heartbeat suppressed; immediate show, visible periodic heartbeat and hide cleanup. Native visibility is simulated.');
} finally {await browser.close();}
