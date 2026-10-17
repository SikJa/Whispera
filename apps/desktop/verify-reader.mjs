import assert from 'node:assert/strict';
import {mkdirSync} from 'node:fs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
 const page=await browser.newPage({viewport:{width:1100,height:850}});
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{
  const old=window.__TAURI_INTERNALS__.invoke;
  window.transcripts=[{id:'reader-fixture',timestamp:'2026-10-17T00:00:00Z',text:'Texto de historial'}];
  window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
   if(command==='snapshot'){const snapshot=await old(command,args);return {...snapshot,history:window.transcripts};}
   if(command==='read_history')return window.transcripts;
   return old(command,args);
  };
 });
 await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/');
 await page.getByRole('button',{name:'Lectura',exact:true}).click();
 const read=page.getByRole('button',{name:'Leer en voz alta',exact:true});
 await page.getByLabel('Voz de lectura').selectOption('fixture-es');assert.equal(await read.isDisabled(),true);
 await page.getByLabel('Texto para escuchar').fill('Solo debe leerse al pulsar el botón.');
 assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='reader_speak').length),0);
 await read.click();await page.getByText('Leyendo en voz alta.',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Pausar',exact:true}).click();await page.getByText('Lectura pausada.',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Continuar',exact:true}).click();await page.getByText('Leyendo en voz alta.',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Detener',exact:true}).click();await page.waitForFunction(()=>window.readerStatus.phase==='idle');
 assert.ok(!await page.evaluate(()=>JSON.stringify(localStorage).includes('Solo debe leerse')));
 await page.evaluate(()=>{window.readerFailure=true;});await read.click();await page.getByRole('alert').getByText('Error: Terminá el dictado antes de leer.').waitFor();
 await page.evaluate(()=>{window.readerFailure=false;});
 await page.getByRole('button',{name:'Historial',exact:true}).click();await page.locator('.history-row').first().click();
 await page.getByLabel('Texto de transcripción').fill('Texto corregido para escuchar.');
 await page.getByRole('button',{name:'Leer en voz alta',exact:true}).click();
 assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='reader_speak').at(-1).args.text),'Texto corregido para escuchar.');
 await page.getByRole('button',{name:'Cerrar transcripción'}).click();await page.waitForFunction(()=>window.readerStatus.phase==='idle');
 await page.getByRole('button',{name:'Lectura',exact:true}).click();
 for(const width of [1100,620]){
  await page.setViewportSize({width,height:850});
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
 }
 mkdirSync('../../.local',{recursive:true});
 await page.screenshot({path:'../../.local/reader-preview.png',fullPage:true});
 const setup=await browser.newPage({locale:"es-ES"});await setup.addInitScript(installNativeMock,{setupComplete:false});
 await setup.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/');
 await setup.getByRole('button',{name:'Usar lectura sin Groq'}).click();await setup.getByRole('button',{name:'Lectura',exact:true}).click();
 assert.ok(await setup.evaluate(()=>window.calls.some(c=>c.command==='complete_setup'&&c.args.readingOnly===true)));
 assert.ok(!await setup.evaluate(()=>window.calls.some(c=>c.command==='validate_key')));
 assert.deepEqual(errors,[]);
 console.log('PASS: local reader explicit play, pause/resume/stop, empty input, privacy, blocked dictation, edited history, panel cleanup, responsive layout and setup without Groq (mocked IPC; no audio).');
} finally {await browser.close();}
