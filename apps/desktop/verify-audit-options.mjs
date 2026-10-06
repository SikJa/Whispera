import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});await server?.listen();
const browser=await launchSilentBrowser();const cases=[];
try{
 const page=await browser.newPage();page.setDefaultTimeout(15000);const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.addInitScript(installNativeMock,{});
 await page.addInitScript(()=>{
  window.settings=JSON.parse(localStorage.getItem('audit.options')||'null')||{model:'whisper-large-v3-turbo',language:'es',hotkey:'Alt+Z',color:'#9024DC',pattern:'wave',placement:'right',autoCopy:true,autoPaste:true,soundTheme:'cristal',sounds:true,recorderScale:.85,trimSilence:true,watchdog:true,incrementalTranscription:true,dictationArtwork:'original'};
  window.rules=[];window.samples=[];const play=HTMLMediaElement.prototype.play;HTMLMediaElement.prototype.play=function(...args){window.samples.push(this);return play.apply(this,args);};
  const original=window.__TAURI_INTERNALS__.invoke;window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
   if(command==='snapshot')return{...await original(command,args),settings:structuredClone(window.settings),rules:structuredClone(window.rules)};
   if(command==='read_settings')return structuredClone(window.settings);
   if(command==='save_settings'){window.settings=args.settings;localStorage.setItem('audit.options',JSON.stringify(args.settings));return;}
   if(command==='save_rules'){window.rules=args.rules;return;}
   return original(command,args);
  };
 });
 await page.goto(process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`,{timeout:90000});
 const saved=async(key,value)=>{await page.waitForFunction(({key,value})=>window.settings[key]===value,{key,value});cases.push({key,value,result:'passed'});};
 for(const model of ['whisper-large-v3-turbo','whisper-large-v3'])for(const lang of ['es','en','pt','auto']){await page.locator('#model').selectOption(model);await page.locator('#language').selectOption(lang);await saved('model',model);await saved('language',lang);}
 const flags=[['Copiar al finalizar','autoCopy'],['Pegar al finalizar dictado','autoPaste'],['Transcripcion anticipada','incrementalTranscription'],['Recortar silencios','trimSilence']];
 for(let mask=0;mask<16;mask++)for(const [index,[label,key]] of flags.entries()){const value=!!(mask&(1<<index));const control=page.getByRole('switch',{name:label,exact:true});if((await control.getAttribute('aria-checked')==='true')!==value)await control.click();await saved(key,value);}
 await page.getByRole('button',{name:'Apariencia',exact:true}).click();
 for(const pattern of ['stairs','wave'])for(const placement of ['left','right','top','bottom']){await page.locator('#pattern').selectOption(pattern);await page.locator('#placement').selectOption(placement);await saved('pattern',pattern);await saved('placement',placement);}
 for(const scale of [.6,.85,1.25]){await page.locator('#recorder-scale').fill(String(scale));await saved('recorderScale',scale);const pct=await page.locator('#recorder-scale').evaluate(e=>Number(e.style.getPropertyValue('--range-progress').replace('%','')));assert.ok(Math.abs(pct-(scale-.6)/.65*100)<.001);}
 await page.getByRole('button',{name:'Sonidos',exact:true}).click();const themes=await page.locator('#sound-theme option').evaluateAll(xs=>xs.map(x=>x.value));
 for(const theme of themes){const count=await page.evaluate(()=>window.samples.length);await page.locator('#sound-theme').selectOption(theme);await saved('soundTheme',theme);assert.equal(await page.evaluate(()=>window.samples.length),count);for(const cue of ['inicio','fin']){await page.getByRole('button',{name:'Escuchar '+cue,exact:true}).click();await page.waitForFunction(()=>window.samples.at(-1)?.readyState>=2);assert.equal(await page.evaluate(()=>window.samples.at(-1).error),null);assert.ok(await page.evaluate(()=>window.samples.slice(0,-1).every(x=>x.paused)));}}
 for(const value of [false,true]){const control=page.getByRole('switch',{name:'Activar sonidos'});if((await control.getAttribute('aria-checked')==='true')!==value)await control.click();await saved('sounds',value);}
 await page.getByRole('button',{name:'Diccionario personal',exact:true}).click();const source=page.getByRole('textbox',{name:'Palabra detectada',exact:true}),target=page.getByRole('textbox',{name:'Corrección',exact:true});
 await source.fill('test');await target.fill('TEST');await page.getByRole('button',{name:'Agregar corrección'}).click();await page.getByRole('status').filter({hasText:'La corrección debe cambiar la palabra.'}).waitFor();assert.equal(await page.evaluate(()=>window.rules.length),0);
 await target.fill('Prueba Ñ');await page.getByRole('button',{name:'Agregar corrección'}).click();await page.getByRole('button',{name:'Editar test',exact:true}).waitFor();
 await source.fill('TEST');await target.fill('Duplicada');await page.getByRole('button',{name:'Agregar corrección'}).click();await page.getByRole('status').filter({hasText:'Ya existe una regla para esa palabra.'}).waitFor();assert.equal(await page.evaluate(()=>window.rules.length),1);
 await page.getByRole('button',{name:'Editar test',exact:true}).click();await target.fill('Cambio descartado');await page.getByRole('button',{name:'Cancelar edición',exact:true}).click();assert.equal(await page.evaluate(()=>window.rules[0].target),'Prueba Ñ');
 await page.getByRole('button',{name:'Editar test',exact:true}).click();await target.fill('Prueba editada');await page.getByRole('button',{name:'Guardar corrección',exact:true}).click();await page.waitForFunction(()=>window.rules[0].target==='Prueba editada');
 await page.getByRole('checkbox',{name:'Activar test',exact:true}).uncheck();await page.waitForFunction(()=>window.rules[0].enabled===false);
 await page.getByRole('textbox',{name:'Buscar corrección'}).fill('EDITADA');assert.equal(await page.locator('.rule-row').count(),1);await page.getByRole('textbox',{name:'Buscar corrección'}).fill('sin coincidencia');assert.equal(await page.locator('.rule-row').count(),0);await page.getByRole('textbox',{name:'Buscar corrección'}).fill('');
 await page.getByRole('button',{name:'Eliminar test',exact:true}).click();await page.waitForFunction(()=>window.rules.length===0);cases.push({key:'dictionary',value:'same/duplicate rejected; edit/cancel/toggle/search/delete synthetic',result:'passed'});
 const final=await page.evaluate(()=>window.settings);await page.reload();await page.locator('#model').waitFor();assert.deepEqual(await page.evaluate(()=>window.settings),final);assert.deepEqual(errors,[]);
 writeFileSync('../../outputs/auditoria-2026-10-06/evidencias/options-matrix.json',JSON.stringify({method:'React UI, mocked native persistence; media files decoded silently; no native/global changes',cases,themes,persistence:'reload passed'},null,2));
 console.log(`PASS: ${cases.length} discrete-value assertions, 16 boolean combinations, all models/languages/placements/patterns, scale boundaries, ${themes.length*2} decoded sound samples, dictionary errors and recovery; simulated native IPC.`);
}finally{await browser.close();await server?.close();}
