import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server.listen();
const browser=await launchSilentBrowser();
try {
 const page=await browser.newPage({viewport:{width:520,height:520}});
 await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{
  const old=window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke=async(c,a)=>{
   if(c==='recording_state')return{phase:'paused',seconds:19,error:'',text:'',muted:false,progress:''};
   if(c==='read_settings')return{...(await old('snapshot')).settings,dictationArtwork:'metallic',metallicColor:'#ffffff',metallicOriginal:true,recorderScale:.85,placement:'right'};
   if(c==='recorder_size'||c==='recorder_region')return null;
   return old(c,a);
  };
 });
 await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/?view=record`);
 await page.getByRole('button',{name:'Reanudar',exact:true}).waitFor();
 await page.waitForTimeout(1200);
 for(const name of ['Transcribir audio','Configuración','Cancelar grabación','Ocultar grabadora'])assert.equal(await page.getByRole('button',{name,exact:true}).count(),0);
 for(const name of ['Reanudar','Detener grabación','Mutear sonido']){
  const box=await page.getByRole('button',{name,exact:true}).boundingBox();
  assert.ok(box.x>=0&&box.x+box.width<=520&&box.y>=0&&box.y+box.height<=520);
 }
 await page.getByRole('button',{name:'Reanudar',exact:true}).click();
 assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='recording_action'&&c.args.action==='pause')));
 await page.screenshot({path:'../../.local/floating-compact-19.png',omitBackground:true});
 console.log('PASS: compact metallic recorder, three functional controls, removed auxiliary buttons and bounded hit regions. Native IPC mocked.');
}finally{await browser.close();await server.close();}
