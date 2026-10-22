import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
 const page=await browser.newPage({viewport:{width:760,height:720}});await page.addInitScript(installNativeMock,{windowLabel:'recorder'});
 await page.addInitScript(()=>{
  window.rectReads=0;const originalRect=Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect=function(...args){if(this.closest('.floating-recorder'))window.rectReads++;return originalRect.apply(this,args);};
  window.voiceState={phase:'idle',seconds:0,error:'',text:'',muted:false,progress:'',session:'synthetic',level:0};
  const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
   if(command==='recording_state'){window.calls.push({command,args});return structuredClone(window.voiceState);}
   if(command==='pending_recordings'){window.calls.push({command,args});return [];}
   return original(command,args);
  };
 });
 const base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';await page.goto(base+'/?view=record');await page.locator('.floating-recorder[data-phase="idle"]').waitFor();await page.waitForTimeout(2000);
 const count=()=>page.evaluate(()=>({rectReads:window.rectReads,stateCalls:window.calls.filter(call=>call.command==='recording_state').length,settingsCalls:window.calls.filter(call=>call.command==='read_settings').length}));
 const start=await count();await page.waitForTimeout(4000);const visible=await count();
 if(process.env.WHISPERA_PERF_RESULTS)await page.screenshot({path:process.env.WHISPERA_PERF_RESULTS.replace(/\.json$/,'.png')});
 await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,get:()=>true});Object.defineProperty(document,'visibilityState',{configurable:true,get:()=> 'hidden'});document.dispatchEvent(new Event('visibilitychange'));});
 await page.waitForTimeout(100);const hiddenStart=await count();await page.waitForTimeout(4000);const hidden=await count();
 const resumedAt=Date.now();await page.evaluate(()=>{window.voiceState.phase='recording';window.voiceState.seconds=65;Object.defineProperty(document,'hidden',{configurable:true,get:()=>false});Object.defineProperty(document,'visibilityState',{configurable:true,get:()=> 'visible'});document.dispatchEvent(new Event('visibilitychange'));});
 await page.locator('.floating-recorder[data-phase="recording"]').waitFor();const resumeMs=Date.now()-resumedAt;
 const result={durationMs:4000,visible:{rectReads:visible.rectReads-start.rectReads,stateCalls:visible.stateCalls-start.stateCalls},hidden:{stateCalls:hidden.stateCalls-hiddenStart.stateCalls,settingsCalls:hidden.settingsCalls-hiddenStart.settingsCalls},resumeMs,nativeIpc:'mocked'};
 if(process.env.WHISPERA_PERF_RESULTS)writeFileSync(process.env.WHISPERA_PERF_RESULTS,JSON.stringify(result,null,2));
 if(process.env.WHISPERA_PERF_MODE!=='baseline'){
  assert.equal(result.hidden.stateCalls,0);assert.equal(result.hidden.settingsCalls,0);assert.ok(result.visible.rectReads<15,JSON.stringify(result));assert.ok(resumeMs<750,JSON.stringify(result));
  await page.getByLabel('Tiempo grabado').filter({hasText:'01:05'}).waitFor();
  const before=await page.evaluate(()=>window.calls.filter(call=>call.command==='recorder_region').length);
  await page.evaluate(()=>document.querySelector('.float-rig').style.transform='translateX(20px)');
  await page.waitForFunction(count=>window.calls.filter(call=>call.command==='recorder_region').length>count,before);
 }
 console.log('PASS performance: '+JSON.stringify(result));
} finally {await browser.close();}
