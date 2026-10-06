import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
const base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';
try {
  const hud=await browser.newPage({viewport:{width:260,height:56}});
  await hud.addInitScript(installNativeMock,{kind:'image'});
  await hud.addInitScript(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='screen_editor_context')return new Promise(resolve=>{window.finishContext=()=>resolve({id:'old-session',kind:'image',width:100,height:100,scale:1});});
      if(command==='screen_editor_feedback_get')return new Promise(resolve=>{window.finishFeedback=()=>resolve({busy:true,error:'Error viejo'});});
      if(command==='screen_editor_action'&&window.failAction)throw Error('Vista incorrecta');
      return original(command,args);
    };
  });
  await hud.goto(base+'/overlay.html?view=screen-hud');
  await hud.waitForFunction(()=>!!window.finishContext);
  await hud.evaluate(()=>window.emitNative('screen-editor-reset',{...window.editorContext,id:'current-session'}));
  await hud.getByLabel('Acciones de captura').waitFor();
  await hud.waitForFunction(()=>!!window.finishFeedback);
  await hud.evaluate(()=>{
    window.emitNative('screen-editor-feedback',{id:'current-session',feedback:{busy:false,error:''}});
    window.finishFeedback();window.finishContext();
  });
  await hud.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready'));
  assert.equal(await hud.getByRole('alert').count(),0,'late initial feedback cannot replace live feedback');
  await hud.getByRole('button',{name:'Copiar',exact:true}).click();
  assert.equal(await hud.evaluate(()=>window.calls.findLast(c=>c.command==='screen_editor_action').args.id),'current-session','late initial context cannot replace the resized/current capture');
  await hud.evaluate(()=>{window.failAction=true;});
  await hud.getByRole('button',{name:'Copiar',exact:true}).click();
  await hud.getByRole('alert').waitFor();
  await hud.evaluate(()=>{window.failAction=false;});
  await hud.getByRole('button',{name:'Cerrar aviso'}).click();
  await hud.getByRole('button',{name:'Copiar',exact:true}).click();
  assert.equal(await hud.getByRole('alert').count(),0);

  const ink=await browser.newPage({viewport:{width:640,height:480}});
  await ink.addInitScript(installNativeMock,{kind:'image'});
  await ink.addInitScript(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='screen_editor_image'&&window.failImage)throw Error('Prueba: imagen no disponible');
      return original(command,args);
    };
  });
  await ink.goto(base+'/overlay.html?view=screen-ink');
  await ink.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready')&&!window.feedback.busy);
  await ink.evaluate(()=>{window.failImage=true;window.emitNative('screen-editor-reset',{...window.editorContext,width:300,height:200});});
  await ink.waitForFunction(()=>window.feedback.error?.includes('imagen no disponible'));
  await ink.evaluate(()=>{window.failImage=false;window.emitNative('screen-editor-reset',{...window.editorContext,width:320,height:240});});
  await ink.waitForFunction(()=>!window.feedback.busy&&!window.feedback.error);
  assert.equal(await ink.evaluate(()=>window.calls.filter(c=>c.command==='screen_editor_ready').length),1,'moving/resizing an already visible image must not focus it again');
  assert.equal(await ink.getByLabel('Editar captura').getAttribute('width'),'320');
  const tools=await browser.newPage({viewport:{width:380,height:600}});
  await tools.addInitScript(installNativeMock,{kind:'image'});
  await tools.goto(base+'/overlay.html?view=screen-tools');
  await tools.getByRole('button',{name:'Elegir color'}).click();
  await tools.getByRole('dialog',{name:'Color del trazo',exact:true}).waitFor();
  await tools.evaluate(()=>window.emitNative('screen-editor-drag',window.editorContext.id));
  await tools.getByRole('dialog',{name:'Color del trazo',exact:true}).waitFor({state:'detached'});
  assert.deepEqual(await tools.getByLabel('Herramientas de captura').evaluate(e=>({x:e.style.left,y:e.style.top})),{x:'4px',y:'4px'},'drag clears expanded-menu offsets');
  await tools.getByRole('button',{name:'Contraer herramientas'}).click();
  await tools.evaluate(()=>window.emitNative('screen-editor-drag',window.editorContext.id));
  assert.equal(await tools.getByLabel('Herramientas de captura').getAttribute('data-compact'),'true','drag preserves the compact toolbar');
  console.log('PASS: late context/feedback, HUD action recovery, image resize recovery and no repeated focus. IPC mocked.');
} finally {await browser.close();}
