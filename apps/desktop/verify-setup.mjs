import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:900,height:760},locale:'es-AR'});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(installNativeMock,{setupComplete:false});
  await page.goto((process.env.WHISPERA_TEST_URL || 'http://127.0.0.1:5190') + '/');
  await page.getByRole('heading',{name:'Tu voz, tu configuración'}).waitFor();
  await page.getByRole('button',{name:'Continuar',exact:true}).click();
  await page.getByText('¿Cómo obtener tu clave de Groq?',{exact:true}).click();
  assert.equal(await page.locator('.guide-step').count(),5);
  for(const img of await page.locator('.guide-step img').all()) {
    await img.scrollIntoViewIfNeeded();
    await img.evaluate(el=>el.decode());
    assert.ok(await img.evaluate(el=>el.naturalWidth>0));
  }
  await page.getByRole('link',{name:'Abrir la consola de Groq ↗'}).click();
  assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='open_groq_console')));
  await page.getByLabel('Groq API key').fill('gsk_test_fixture');
  await page.getByRole('button',{name:'Validar y guardar'}).click();
  await page.getByRole('button',{name:'Continuar',exact:true}).click();
  await page.getByRole('heading',{name:'Prepará tu primer dictado'}).waitFor();
  await page.getByRole('button',{name:'Continuar',exact:true}).click();
  await page.getByRole('button',{name:'Ir a la bandeja'}).click();
  await page.getByRole('button',{name:'Atajos',exact:true}).waitFor();
  assert.equal(await page.getByRole('button',{name:'Guía inicial / Setup'}).count(),0);
  await page.reload();
  await page.getByRole('button',{name:'Atajos',exact:true}).waitFor();
  assert.equal(await page.locator('.setup-shell').count(),0);
  assert.deepEqual(errors,[]);
  console.log('PASS: five illustrated Groq steps inside setup, native console link, completion persisted and setup entry removed after completion (mocked IPC).');
} finally {await browser.close();}
