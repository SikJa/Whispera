import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:1100,height:850}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{
    window.dictionary=[];
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
      if(command==='snapshot'){const data=await original(command,args);return {...data,rules:structuredClone(window.dictionary)};}
      if(command==='read_rules')return structuredClone(window.dictionary);
      if(command==='save_rules'){
        if(JSON.stringify(args.expectedRules)!==JSON.stringify(window.dictionary))throw Error('El diccionario cambio desde que abriste esta pantalla.');
        window.dictionary=structuredClone(args.rules);return;
      }
      return original(command,args);
    };
  });
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/');
  await page.getByRole('heading',{name:'Transcripción',exact:true}).waitFor();
  await page.evaluate(()=>window.dictionary=[{id:'restored',source:'wispara',target:'Whispera',enabled:true}]);
  await page.getByRole('button',{name:'Diccionario personal',exact:true}).click();
  await page.locator('.rule-row').waitFor();
  assert.equal(await page.locator('.rule-row').count(),1,'Entering dictionary reloads restored rules');
  await page.evaluate(()=>window.dictionary.push({id:'external',source:'groc',target:'Groq',enabled:true}));
  await page.getByLabel('Palabra detectada',{exact:true}).fill('codexx');
  await page.getByLabel('Corrección',{exact:true}).fill('Codex');
  await page.getByRole('button',{name:'Agregar corrección',exact:true}).click();
  await page.getByText(/El diccionario cambio/).waitFor();
  assert.equal(await page.locator('.rule-row').count(),2,'Conflict reloads current dictionary');
  assert.equal(await page.getByLabel('Palabra detectada',{exact:true}).inputValue(),'codexx','Failed save keeps typed changes');
  await page.getByRole('button',{name:'Agregar corrección',exact:true}).click();
  await page.getByText('Corrección guardada',{exact:true}).waitFor();
  assert.equal(await page.locator('.rule-row').count(),3);
  await page.getByRole('button',{name:'Eliminar codexx',exact:true}).click();
  await page.getByText('Regla eliminada',{exact:true}).waitFor();
  assert.equal(await page.locator('.rule-row').count(),2);
  assert.deepEqual(errors,[]);
  console.log('PASS dictionary restores on entry, rejects stale replacement, reloads conflict, keeps input and preserves other rules on retry/delete; mocked IPC.');
} finally {await browser.close();}
