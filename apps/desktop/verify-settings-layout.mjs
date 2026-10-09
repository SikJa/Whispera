import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});
await server?.listen();
const browser=await launchSilentBrowser();
try{
  const page=await browser.newPage({viewport:{width:1100,height:720}});
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    const history=Array.from({length:180},(_,i)=>({id:String(i),timestamp:'2026-10-06T00:00:00Z',text:`Transcripción de prueba ${i}`}));
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='snapshot'){const data=await original(command,args);return{...data,settings:{...data.settings,recorderScale:.85,pattern:'wave',sounds:true,soundTheme:'cristal'},history};}
      if(command==='read_history')return history;
      if(command==='settings_window_action'){window.calls.push({command,args});if(args.action==='maximize')window.mockMaximized=!window.mockMaximized;return !!window.mockMaximized;}
      return original(command,args);
    };
  });
  await page.goto(process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`);
  assert.equal(await page.getByRole('button',{name:'General',exact:true}).count(),0,'empty General route must not return');
  await page.getByRole('button',{name:'Historial',exact:true}).click();
  const sidebar=page.locator('.settings-sidebar');
  const top=(await sidebar.boundingBox()).y;
  const main=page.getByRole('region',{name:'Contenido de configuración'});
  await main.evaluate(e=>{e.scrollTop=e.scrollHeight;});
  assert.equal((await sidebar.boundingBox()).y,top,'scroll must not move the sidebar');
  assert.equal(await page.evaluate(()=>document.scrollingElement.scrollTop),0);
  for(let i=0;i<3;i++){
    await page.getByRole('button',{name:'Capturas y videos',exact:true}).click();
    await page.getByRole('button',{name:'Transcripciones',exact:true}).click();
  }
  assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='screen_recent').length),1,'history tabs should keep their loaded list');
  for(const name of ['Sonidos','Capturas y video','Apariencia','Historial','Actualizaciones']){
    await page.getByRole('button',{name,exact:true}).click();
    assert.equal((await sidebar.boundingBox()).y,top);
    assert.equal(await page.locator('.settings-section').evaluate(e=>getComputedStyle(e).opacity),'1','section must never start invisible');
  }
  await page.getByRole('button',{name:'Apariencia',exact:true}).click();
  const cameraSize=await page.locator('.appearance-indicators .indicator-artwork').boundingBox();
  assert.ok(await page.locator('#recorder-scale').evaluate(e=>e.closest('.form-row').textContent.includes('Tamaño de carpeta')));
  assert.ok((await page.locator('#recorder-scale').boundingBox()).y<cameraSize.y,'folder size belongs above the fixed camera section');
  await page.locator('#recorder-scale').focus();await page.keyboard.press('Home');
  assert.equal(await page.locator('#recorder-scale').inputValue(),'0.6');
  assert.equal(await page.locator('#recorder-scale').evaluate(e=>e.style.getPropertyValue('--range-progress')),'0%');
  await page.getByText('Mín. 60 %',{exact:true}).waitFor();
  await page.keyboard.press('End');assert.equal(await page.locator('#recorder-scale').inputValue(),'1.25');
  assert.equal((await page.locator('.appearance-indicators .indicator-artwork').boundingBox()).width,cameraSize.width,'folder scaling never changes the camera');
  await page.getByRole('button',{name:'Contraer sidebar',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.settings-sidebar').getBoundingClientRect().width<65);
  await sidebar.getByRole('button',{name:'Sonidos',exact:true}).hover();
  await page.locator('.sidebar-tooltip').waitFor();
  assert.ok((await page.locator('.sidebar-tooltip').boundingBox()).x>(await sidebar.boundingBox()).x+64,'tooltip must not be clipped inside the rail');
  await sidebar.getByRole('button',{name:'Sonidos',exact:true}).click();
  await page.getByRole('heading',{name:'Sonidos',exact:true}).waitFor();
  await page.getByRole('button',{name:'Maximizar ventana'}).click();
  await page.getByRole('button',{name:'Restaurar ventana'}).waitFor();
  await page.getByRole('button',{name:'Minimizar ventana'}).click();
  await page.getByRole('button',{name:'Cerrar configuración'}).click();
  assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='settings_window_action'&&c.args.action==='close')));
  await page.getByRole('button',{name:'Expandir sidebar',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.settings-sidebar').getBoundingClientRect().width>223);
  await page.screenshot({path:'../../.local/settings-0.2.18.png'});
  await page.setViewportSize({width:620,height:520});
  await page.getByRole('button',{name:'Historial',exact:true}).click();
  await main.evaluate(e=>{e.scrollTop=e.scrollHeight;});
  await sidebar.locator('nav').evaluate(e=>{e.scrollTop=e.scrollHeight;});
  assert.equal(await page.evaluate(()=>document.scrollingElement.scrollHeight),520,'no outer scrolling at minimum window size');
  await page.screenshot({path:'../../.local/settings-0.2.18-small.png'});
  console.log('PASS: fixed shell, internal scrolling, persistent history, silent route changes, bounded slider, collapsible navigation, tooltips and window controls. Native IPC mocked.');
}finally{await browser.close();await server?.close();}
