import assert from 'node:assert/strict';
import {mkdirSync} from 'node:fs';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});
await server?.listen();
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:1000,height:780}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  page.on('console',message=>{if(message.type()==='error')console.error('Browser:',message.text());});
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{
    window.updateStatus.currentVersion='0.3.0-readaloud.10';window.updateStatus.version='99.0.0';
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='ai_usage_snapshot'){window.calls.push({command,args});return {providers:[],claudeConnection:{connected:false,settingsPath:''}};}
      return original(command,args);
    };
  });
  const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
  await page.goto(base,{timeout:90000,waitUntil:'domcontentloaded'});
  const search=page.getByRole('combobox',{name:'Buscar opciones',exact:true});
  await search.waitFor();
  const find=async(query,title,heading,selector)=>{
    console.log(`Search destination: ${title}`);
    await search.fill(query);
    await page.getByRole('option').filter({has:page.getByText(title,{exact:true})}).click();
    await page.getByRole('heading',{name:heading,exact:true}).waitFor();
    assert.equal(await search.inputValue(),'');
    await page.waitForFunction(selector=>document.querySelector(selector)?.closest('[data-search-highlight]')!==null&&!!document.querySelector(selector),selector);
  };
  await find('version','Versión instalada de Whispera','Aplicación','#installed-version');
  assert.equal(await page.locator('#installed-version strong').innerText(),'0.3.0-readaloud.10','Display the installed version, never the available update');
  await find('MICROFONO','Audio del video','Capturas y video','[aria-label="Sonido del video"]');
  await find('atajo lectura','Atajo de lectura','Atajos','#shortcut-reading');
  assert.equal(await page.locator('.hotkey-field[data-listening=true]').count(),0,'Search navigation must not start recording a shortcut');
  await find('historial privacidad','Pausar el historial','Portapapeles','#library-incognito');
  assert.equal(await page.getByRole('tab',{name:'Configuración',exact:true}).getAttribute('aria-selected'),'true','Open the nested settings tab');
  await find('retencion','Eliminar automáticamente','Portapapeles','#library-retention');
  await find('saldo','Cuotas de OpenAI, Codex y Claude','Uso de IA','.ai-providers');
  await find('fondo','Fondo personalizado','Apariencia','.app-background-picker');
  await search.fill('xyz_no_existe_123');
  await page.getByText('Sin resultados. Probá con voz, audio, atajo o versión.',{exact:true}).waitFor();
  assert.equal(await page.locator('.settings-search-results [role=option]').count(),0);
  await search.press('Escape');assert.equal(await search.getAttribute('aria-expanded'),'false');
  await search.fill('velocidad');await search.press('Enter');
  await page.getByRole('heading',{name:'Lectura',exact:true}).waitFor();
  await search.fill('tema');await search.press('ArrowDown');
  const active=await search.getAttribute('aria-activedescendant');
  assert.equal(await page.locator(`[id="${active}"]`).getAttribute('aria-selected'),'true');
  await search.press('Escape');
  await page.getByRole('heading',{name:'Lectura',exact:true}).click();
  await page.keyboard.press('Control+k');assert.equal(await search.evaluate(e=>e===document.activeElement),true);
  await search.fill('version');await page.locator('.window-chrome-caption').click();assert.equal(await search.getAttribute('aria-expanded'),'false');
  await page.getByRole('button',{name:'Contraer sidebar',exact:true}).click();
  await find('version','Versión instalada de Whispera','Aplicación','#installed-version');
  for(const width of [620,420]){
    await page.setViewportSize({width,height:760});await search.fill('audio');
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true,'No horizontal overflow');
    const box=await page.locator('.settings-search-results').boundingBox();assert.ok(box.x>=0&&box.x+box.width<=width);
    await search.press('Escape');
  }
  await page.setViewportSize({width:1000,height:780});
  await search.fill('microfono');mkdirSync('../../.local',{recursive:true});
  await page.screenshot({path:'../../.local/settings-search-preview.png'});
  const calls=await page.evaluate(()=>window.calls);
  assert.equal(calls.some(c=>c.command.startsWith('save_')||['reader_action','reader_export','screen_select','screen_select_image','updater_check','updater_install'].includes(c.command)),false,'Navigation must not write preferences, start media or check updates');
  assert.deepEqual(errors,[]);
  const preview=await browser.newPage();await preview.goto(base,{timeout:90000,waitUntil:'domcontentloaded'});await preview.getByRole('button',{name:'Aplicación',exact:true}).click();
  assert.equal(await preview.locator('#installed-version strong').innerText(),'Vista previa','Browser preview must not pretend to be an installed release');
  console.log('PASS: settings search, aliases, accents, exact and nested destinations, installed version, keyboard, compact layout and read-only navigation. Native IPC mocked.');
} finally {await browser.close();await server?.close();}
