import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();
const browser=await launchSilentBrowser();
try{
  const page=await browser.newPage({viewport:{width:420,height:360}});
  await page.addInitScript(installNativeMock,{windowLabel:'import'});
  await page.addInitScript(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='plugin:dialog|open')return 'C:\\QA\\audio.wav';
      if(command==='transcribe_file')return new Promise(resolve=>{window.finishImport=resolve;});
      return original(command,args);
    };
  });
  await page.goto((process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`)+'/?view=import',{timeout:90000});
  const fits=async()=>assert.equal(await page.locator('.file-import').evaluate(element=>element.scrollHeight<=element.clientHeight+1&&element.scrollWidth<=element.clientWidth+1),true,'Import content fits smallest supported window');
  await page.getByText('Arrastrá un audio acá').waitFor();await fits();
  await page.getByRole('button',{name:'Explorar',exact:true}).click();
  await page.getByText('Transcribiendo',{exact:true}).waitFor();await fits();
  await page.evaluate(()=>window.finishImport('Transcripción de prueba. '.repeat(200)));
  await page.getByText('Transcripción lista',{exact:true}).waitFor();await fits();
  await page.getByRole('button',{name:'Editar',exact:true}).click();
  const editor=page.getByRole('textbox',{name:'Editar transcripción importada'});await editor.waitFor();await fits();
  assert.ok((await editor.inputValue()).length>1000,'Long results remain editable');
  await editor.fill('Corrección conservada');await page.getByRole('button',{name:'Copiar',exact:true}).click();
  assert.equal(await page.evaluate(()=>window.calls.find(call=>call.command==='copy_text')?.args.text),'Corrección conservada');
  await page.getByRole('button',{name:'Otro audio',exact:true}).click();await page.getByText('Arrastrá un audio acá').waitFor();await fits();
  await page.getByRole('link',{name:'Whispera',exact:true}).click();
  await page.waitForFunction(()=>window.calls.some(call=>call.command==='open_settings'));
  assert.ok(await page.evaluate(()=>window.calls.some(call=>call.command==='plugin:window|hide')),'Return hides import and opens the canonical settings window');
  assert.equal(new URL(page.url()).searchParams.get('view'),'import','Never render settings inside the small import window');
  console.log('PASS: compact import without outer scrolling at 420×360; browse, processing, long result editing, copy and reset. Native IPC mocked.');
}finally{await browser.close();await server?.close();}
