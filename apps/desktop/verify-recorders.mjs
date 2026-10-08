import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();const browser=await launchSilentBrowser();
try{
 const page=await browser.newPage({viewport:{width:760,height:720}});await page.addInitScript(installNativeMock,{windowLabel:'recorder'});
 await page.addInitScript(()=>{const original=window.__TAURI_INTERNALS__.invoke;window.voiceState={phase:'idle',seconds:0,error:'',text:'',muted:false,progress:'',session:'audit',level:0};window.voiceSettings={color:'#112233',recorderScale:.85,pattern:'wave',dictationArtwork:'metallic',metallicOriginal:true,placement:'right'};window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
  if(['recording_state','read_settings','pending_recordings','recording_action'].includes(command)){window.calls.push({command,args});
   if(command==='recording_state')return structuredClone(window.voiceState);if(command==='read_settings')return structuredClone(window.voiceSettings);if(command==='pending_recordings')return[];
   if(args.action==='start'){window.voiceState.phase='recording';window.voiceState.seconds=61;}
   if(args.action==='pause')window.voiceState.phase=window.voiceState.phase==='paused'?'recording':'paused';
   if(args.action==='mute')window.voiceState.muted=!window.voiceState.muted;
   if(args.action==='stop')window.voiceState.phase='processing';if(args.action==='save')window.voiceState.phase='ready';return;
  }return original(command,args);
 };});
 const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
 await page.goto(base+'/?view=record',{timeout:90000});await page.getByRole('button',{name:'Grabar',exact:true}).click();
 await page.getByLabel('Tiempo grabado').filter({hasText:'01:01'}).waitFor();
 await page.getByRole('button',{name:'Pausar',exact:true}).click();await page.getByRole('button',{name:'Reanudar',exact:true}).click();
 await page.getByRole('button',{name:'Mutear sonido',exact:true}).click();await page.getByRole('button',{name:'Restaurar sonido',exact:true}).click();
 await page.getByRole('button',{name:'Detener grabación',exact:true}).click();await page.locator('.floating-recorder[data-phase="processing"]').waitFor();
 await page.evaluate(()=>{window.voiceState={...window.voiceState,phase:'done',text:'Texto sintético'};window.voiceSettings={...window.voiceSettings,recorderScale:1.25};});
 await page.getByRole('button',{name:'Copiar transcripción',exact:true}).click();assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='copy_recording')));
 await page.waitForFunction(()=>document.querySelector('.floating-recorder').style.transform==='scale(1.25)');
 await page.evaluate(()=>{window.voiceState={...window.voiceState,phase:'error',error:'Prueba de recuperación'};});await page.getByRole('alert').click();assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='open_recording_details')));
 await page.goto(base+'/?view=details',{timeout:90000});await page.getByRole('button',{name:'Grabar',exact:true}).click();
 await page.getByRole('button',{name:'Guardar sin transcribir',exact:true}).click();await page.getByText('Audio guardado',{exact:true}).waitFor();
 await page.evaluate(()=>window.voiceState={...window.voiceState,phase:'done',text:'Texto sintético original'});
 await page.getByRole('button',{name:'Editar texto',exact:true}).click();await page.getByRole('textbox',{name:'Texto transcrito',exact:true}).fill('Corrección sintética');
 await page.getByRole('button',{name:'Guardar en historial',exact:true}).click();assert.equal(await page.evaluate(()=>window.calls.find(c=>c.command==='save_transcript').args.text),'Corrección sintética');
 await page.getByRole('link',{name:'Whispera',exact:true}).click();await page.waitForFunction(()=>window.calls.some(c=>c.command==='plugin:window|hide'));assert.equal(new URL(page.url()).searchParams.get('view'),'details');
 console.log('PASS: recorder state transitions, pause/resume/mute, copy, size refresh, errors, saved audio, corrected history and return to canonical settings. IPC mocked, no microphone.');
}finally{await browser.close();await server?.close();}
