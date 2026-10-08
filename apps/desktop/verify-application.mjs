import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});
await server?.listen();
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:1000,height:720}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='toggle_recorder'){window.recorderVisible=!window.recorderVisible;window.calls.push({command,args});return;}
      if(command==='startup_enabled')return localStorage.getItem('qa-startup')!=='false';
      if(command==='set_startup'){
        window.calls.push({command,args});
        if(window.failStartup)throw Error('Windows rechazó el cambio');
        localStorage.setItem('qa-startup',String(args.enabled));return args.enabled;
      }
      return original(command,args);
    };
  });
  const url=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
  await page.goto(url,{timeout:90000});
  const open=()=>page.getByRole('button',{name:'Aplicación',exact:true}).click();
  const toggle=page.getByRole('switch',{name:'Iniciar Whispera al encender la computadora'});
  await open();await toggle.waitFor();
  await page.waitForFunction(()=>document.querySelector('#app-startup')?.getAttribute('aria-checked')==='true');
  await toggle.click();await page.getByText('Guardado automáticamente.',{exact:true}).waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'false');
  await page.reload();await open();
  await page.waitForFunction(()=>document.querySelector('#app-startup')?.disabled===false);
  assert.equal(await toggle.getAttribute('aria-checked'),'false','Persists when reopened');
  await page.evaluate(()=>{window.failStartup=true;});await toggle.click();
  await page.getByRole('alert').waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'false','Failed save preserves Windows state');
  await page.evaluate(()=>{window.failStartup=false;});await toggle.click();
  await page.getByRole('alert').waitFor({state:'detached'});
  assert.equal(await toggle.getAttribute('aria-checked'),'true');
  const recorder=page.getByRole('link',{name:'Abrir o cerrar grabadora'});
  await recorder.click();await page.waitForFunction(()=>window.recorderVisible===true);
  await page.waitForFunction(()=>document.querySelector('.recorder-link')?.getAttribute('aria-disabled')==='false');
  await recorder.click();await page.waitForFunction(()=>window.recorderVisible===false);
  const space=page.getByRole('button',{name:'Tu espacio',exact:true});
  const dictionary=page.getByRole('button',{name:'Diccionario personal',exact:true});
  assert.equal(await dictionary.locator('xpath=ancestor::div[contains(@class,"sidebar-section")]').getByRole('button',{name:'Tu espacio',exact:true}).count(),1);
  await space.click();assert.equal(await space.getAttribute('aria-expanded'),'false');
  assert.equal(await dictionary.isVisible(),false);
  assert.equal(await page.getByRole('button',{name:'Aplicación',exact:true}).isVisible(),true);
  await page.reload();await space.waitFor();assert.equal(await space.getAttribute('aria-expanded'),'false');
  await space.click();await dictionary.click();await page.getByRole('heading',{name:'Diccionario personal',exact:true}).waitFor();
  assert.deepEqual(errors,[]);
  console.log('PASS: application startup state, immediate saving, reopen persistence and error recovery. Native IPC mocked.');
} finally {await browser.close();await server?.close();}
