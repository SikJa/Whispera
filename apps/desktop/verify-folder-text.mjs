import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();const browser=await launchSilentBrowser();
const near=(actual,expected,message,tolerance=.8)=>assert.ok(Math.abs(actual-expected)<=tolerance,`${message}: ${actual.toFixed(1)} px, expected ${expected.toFixed(1)}`);
try{
 const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;

 // The folder's text and icons are sized in px ON THE SCREEN. They do not follow the folder's own size, so they
 // read the same at any "Tamaño de carpeta" and on any monitor (the system's display scaling already adapts screen px).
 // The notice follows the folder instead: centred under it, as wide as the folder (with room for one readable line),
 // wrapping rather than sticking out, and never leaving the window.
 const messages=['No se detectó micrófono','No hay micrófono disponible','No se pudo cargar el modelo local: el archivo está dañado, descárgalo de nuevo desde Configuración'];
 for(const folder of [.6,.85,1.25])for(const message of messages){
  const page=await browser.newPage({viewport:{width:Math.round(520*folder),height:Math.round(520*folder)}});
  await page.addInitScript(installNativeMock,{windowLabel:'recorder'});
  await page.addInitScript(([folder,message])=>{
   const original=window.__TAURI_INTERNALS__.invoke;
   window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
    if(command==='recording_state')return{phase:'idle',seconds:0,error:message,text:'',muted:false,progress:''};
    if(command==='read_settings')return{color:'#9024DC',recorderScale:folder,pattern:'wave',dictationArtwork:'original',placement:'right'};
    return original(command,args);
   };
  },[folder,message]);
  await page.goto(base+'/?view=record',{timeout:90000});
  await page.getByRole('alert').waitFor();
  await page.waitForFunction(folder=>getComputedStyle(document.querySelector('.floating-recorder')).getPropertyValue('--folder-scale').trim()===String(folder),folder);
  const size=await page.evaluate(()=>{const box=selector=>document.querySelector(selector).getBoundingClientRect();
   const center=rect=>rect.left+rect.width/2;
   return{status:box('.float-readout > span').height,noticeIcon:box('.float-error svg').width,closeIcon:box('.float-error-close svg').width,button:box('.float-actions button').width,buttonIcon:box('.float-actions button svg').width,
    notice:box('.float-error').toJSON(),close:box('.float-error-close').toJSON(),mic:box('.float-actions button').toJSON(),
    folderWidth:box('.float-rig').width,offCenter:center(box('.float-error'))-center(box('.float-rig')),lineHeight:parseFloat(getComputedStyle(document.querySelector('.float-error')).fontSize)*1.15,
    window:{width:innerWidth,height:innerHeight}};});
  const at=`folder ${Math.round(folder*100)} %, "${message.slice(0,24)}"`;
  near(size.status,16,`status line, ${at}`);near(size.noticeIcon,16,`notice icon, ${at}`);near(size.closeIcon,16,`X icon, ${at}`);
  near(size.button,34,`mic button, ${at}`);near(size.buttonIcon,20,`mic icon, ${at}`);
  for(const [name,r] of Object.entries({notice:size.notice,close:size.close,mic:size.mic}))
   assert.ok(r.left>=0&&r.top>=0&&r.right<=size.window.width+.5&&r.bottom<=size.window.height+.5,`${name} stays inside the window, ${at}: ${JSON.stringify(r)} in ${JSON.stringify(size.window)}`);
  near(size.offCenter,0,`notice is centred under the folder, ${at}`,1.5);
  assert.ok(size.notice.width<=Math.max(size.folderWidth,220)+1,`notice is not wider than the folder (or one readable line), ${at}: ${size.notice.width.toFixed(0)} px vs folder ${size.folderWidth.toFixed(0)} px`);
  assert.ok(size.notice.height<=2*size.lineHeight+14,`notice wraps to at most two lines, ${at}: ${size.notice.height.toFixed(0)} px`);
  await page.close();
 }

 console.log('PASS: folder text and icons keep the same size on screen at any folder size; the notice stays centred under the folder, no wider than it, wraps to two lines and never leaves the window. Native IPC mocked.');
}finally{await browser.close();await server?.close();}
