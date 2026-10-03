import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:1100,height:850}});
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{
    let enabled=true;
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='startup_enabled')return enabled;
      if(command==='set_startup'){
        if(window.failStartup)throw Error('No se pudo guardar el inicio');
        enabled=args.enabled;return;
      }
      return original(command,args);
    };
  });
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/');
  await page.getByRole('button',{name:'General',exact:true}).click();
  const toggle=page.getByRole('switch',{name:'Iniciar Whispera con Windows'});
  await page.waitForFunction(()=>document.querySelector('#windows-startup')?.getAttribute('aria-checked')==='true');
  await toggle.click();await page.getByText('Inicio automático desactivado.',{exact:true}).waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'false');
  await page.getByRole('button',{name:'Atajos',exact:true}).click();
  await page.getByRole('button',{name:'General',exact:true}).click();
  await page.waitForFunction(()=>!document.querySelector('#windows-startup')?.disabled);
  assert.equal(await toggle.getAttribute('aria-checked'),'false');
  await toggle.click();await page.getByText('Inicio automático activado.',{exact:true}).waitFor();
  await page.evaluate(()=>window.failStartup=true);
  await toggle.click();await page.getByText(/No se pudo guardar el inicio/).waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'true');
  await page.screenshot({path:'../../.local/startup-settings.png'});
  console.log('PASS startup readback, enable/disable, navigation persistence, failed save preserves state; mocked Windows IPC.');
} finally {await browser.close();}
