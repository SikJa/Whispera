import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
const base=process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190';
try {
  for(const [label,kind,foreign] of [['screen-tools','video','image-tools'],['image-tools','image','screen-tools']]) {
    const page=await browser.newPage({viewport:{width:380,height:600}});
    await page.addInitScript(installNativeMock,{kind,windowLabel:label});
    await page.addInitScript(()=>{
      const original=window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
        if(['screen_editor_ready','screen_tools_panel'].includes(command)&&args.id!==window.editorContext.id)throw Error('Vista incorrecta');
        return original(command,args);
      };
    });
    await page.goto(base+'/overlay.html?view=screen-tools');
    await page.getByLabel('Herramientas de captura').waitFor();
    await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready'));
    await page.evaluate(target=>{
      window.emitNative('screen-editor-reset',{...window.editorContext,id:'foreign-session'},target);
      window.emitNative('screen-editor-feedback',{id:window.editorContext.id,feedback:{error:'Error de otra ventana'}},target);
      window.emitNative('screen-editor-reset',null,target);
    },foreign);
    await page.getByRole('button',{name:'Elegir color'}).click();
    await page.getByRole('dialog',{name:'Color del trazo',exact:true}).waitFor();
    assert.equal(await page.getByLabel('Herramientas de captura').getAttribute('data-session'),'test-session','foreign reset must not replace or close the active toolbar');
    assert.equal(await page.getByRole('alert').count(),0,'foreign events must not produce Vista incorrecta');
    await page.evaluate(target=>window.emitNative('screen-editor-drag',window.editorContext.id,target),foreign);
    assert.equal(await page.getByRole('dialog',{name:'Color del trazo',exact:true}).count(),1,'foreign drag must not close the active panel');
    await page.evaluate(()=>window.emitNative('screen-editor-drag',window.editorContext.id));
    await page.getByRole('dialog',{name:'Color del trazo',exact:true}).waitFor({state:'detached'});
    await page.evaluate(()=>window.emitNative('screen-editor-reset',{...window.editorContext,id:'next-session'}));
    await page.waitForFunction(()=>document.querySelector('.capture-toolbar')?.dataset.session==='next-session');
    await page.close();
  }
  console.log('PASS: video and parallel image toolbars ignore foreign reset, feedback, hide and drag events while accepting their own. IPC mocked.');
} finally {await browser.close();}
