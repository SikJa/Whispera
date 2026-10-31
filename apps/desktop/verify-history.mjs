import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
const diagnostics=[];
try{
  const page=await browser.newPage({viewport:{width:1100,height:800}});
  page.on('pageerror',error=>diagnostics.push(`pageerror: ${error.message}`));
  page.on('requestfailed',request=>diagnostics.push(`requestfailed: ${request.url()} ${request.failure()?.errorText}`));
  function installHistoryMock(){
    const old=window.__TAURI_INTERNALS__.invoke;
    window.transcripts=Array.from({length:120},(_,i)=>({id:String(i),timestamp:'2026-10-02T00:00:00Z',text:`Texto ${i}`}));
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='snapshot'){const data=await old(command,args);return{...data,history:window.transcripts};}
      if(command==='read_history')return window.transcripts;
      if(command==='save_transcript'){window.transcripts=window.transcripts.map(t=>t.id===args.id?{...t,text:args.text}:t);return;}
      return old(command,args);
    };
  }
  // Playwright does not guarantee evaluation order across separate init scripts.
  await page.addInitScript({content:`(${installNativeMock.toString()})({}); (${installHistoryMock.toString()})();`});
  await page.goto((process.env.WHISPERA_TEST_URL || 'http://127.0.0.1:5190') + '/');await page.getByRole('button',{name:'Historial',exact:true}).click();
  assert.equal(await page.locator('.history-row').count(),50);await page.getByRole('button',{name:'Mostrar 50 más'}).click();assert.equal(await page.locator('.history-row').count(),100);
  await page.locator('.history-row').first().click();await page.getByLabel('Texto de transcripción').fill('Texto corregido');
  await page.getByRole('button',{name:'Guardar',exact:true}).click();await page.getByRole('dialog').getByText('Cambios guardados en el historial').waitFor();
  await page.getByRole('button',{name:'Cerrar transcripción'}).click();assert.equal(await page.evaluate(()=>window.transcripts[0].text),'Texto corregido');
  await page.getByLabel('Buscar transcripción').fill('corregido');assert.equal(await page.locator('.history-row').count(),1);
  await page.getByRole('button',{name:'Capturas y videos',exact:true}).click();
  await page.locator('.capture-recent-row').waitFor();await page.locator('.capture-recent-row').getByRole('button',{name:'Copiar',exact:true}).click();
  assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='screen_recent_copy'&&c.args.id==='recent-1')));
  console.log('PASS: history pagination, search, persisted transcript corrections and copying a recent capture (mocked IPC).');
}catch(error){
  console.error('History bootstrap diagnostics:',JSON.stringify(diagnostics));
  for(const page of browser.contexts().flatMap(context=>context.pages())){
    console.error('History page:',page.url(),await page.locator('body').innerText().then(text=>text.slice(0,2000)).catch(()=>'<unavailable>'));
  }
  throw error;
}finally{await browser.close();}
