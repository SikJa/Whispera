import assert from 'node:assert/strict';
import {mkdirSync} from 'node:fs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try{
  const page=await browser.newPage({viewport:{width:1100,height:1000}}),errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(installNativeMock,{});
  await page.addInitScript(()=>{
    const now=Math.floor(Date.now()/1000);
    window.aiUsage={providers:[{id:'codex',name:'OpenAI · Codex',status:'ready',message:'',observedAt:now,plan:'pro',windows:[{label:'Codex · Principal',usedPercent:25,resetAt:now+3600,windowMinutes:300},{label:'Codex · Secundaria',usedPercent:70,resetAt:now+86400,windowMinutes:10080}],credits:[]},{id:'claude',name:'Anthropic · Claude',status:'waiting',message:'Esperando datos de Claude Code.',observedAt:null,plan:null,windows:[],credits:[]}],claudeConnection:{connected:false,settingsPath:'settings.json'}};
    const old=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='ai_usage_snapshot'){window.calls.push({command,args});if(window.quotaFailure)throw Error('No se pudo actualizar');return structuredClone(window.aiUsage);}
      if(command==='ai_usage_connect_claude'){window.calls.push({command,args});window.aiUsage.claudeConnection.connected=args.connect;return structuredClone(window.aiUsage);}
      if(command==='ai_usage_open_provider'){window.calls.push({command,args});return;}
      return old(command,args);
    };
  });
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/');
  await page.getByRole('button',{name:'Uso de IA',exact:true}).click();
  const codex=page.getByRole('article',{name:'OpenAI · Codex'}),claude=page.getByRole('article',{name:'Anthropic · Claude'});
  await codex.getByText('75',{exact:false}).first().waitFor();
  assert.equal(await codex.getByRole('progressbar').first().getAttribute('aria-valuenow'),'75');
  assert.equal(await codex.getByRole('progressbar').nth(1).getAttribute('aria-valuenow'),'30');
  assert.equal(await claude.getByRole('progressbar').count(),0);
  await page.getByRole('button',{name:'Conectar Claude Code',exact:true}).click();
  await page.getByRole('button',{name:'Desconectar Claude Code',exact:true}).waitFor();
  await page.evaluate(()=>{const p=window.aiUsage.providers[1];p.status='ready';p.observedAt=Math.floor(Date.now()/1000)-600;p.windows=[{label:'Sesión',usedPercent:40,resetAt:Math.floor(Date.now()/1000)-1,windowMinutes:300}];});
  await page.getByRole('button',{name:'Actualizar cuotas',exact:true}).click();
  await claude.getByText('Reinicio por confirmar',{exact:true}).waitFor();
  assert.equal(await claude.getByRole('progressbar').getAttribute('aria-valuenow'),null);
  await page.getByRole('button',{name:'Ver uso oficial',exact:true}).first().click();
  assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='ai_usage_open_provider'&&c.args.provider==='codex')));
  await page.getByRole('button',{name:'Desconectar Claude Code',exact:true}).click();
  await page.getByRole('button',{name:'Conectar Claude Code',exact:true}).waitFor();
  await page.evaluate(()=>{window.quotaFailure=true;});await page.getByRole('button',{name:'Actualizar cuotas',exact:true}).click();await page.getByRole('alert').waitFor();
  assert.equal(await codex.getByRole('progressbar').first().getAttribute('aria-valuenow'),'75');
  assert.ok(!await page.evaluate(()=>JSON.stringify(localStorage).includes('usedPercent')));
  for(const width of [1100,620]){await page.setViewportSize({width,height:1000});assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));}
  await page.setViewportSize({width:1100,height:1250});await page.evaluate(()=>{window.quotaFailure=false;window.aiUsage.claudeConnection.connected=true;const p=window.aiUsage.providers[1];p.status='ready';p.observedAt=Math.floor(Date.now()/1000);p.windows=[{label:'Sesión',usedPercent:44,resetAt:Math.floor(Date.now()/1000)+7200,windowMinutes:300},{label:'Semana',usedPercent:18,resetAt:Math.floor(Date.now()/1000)+172800,windowMinutes:10080}];});
  await page.getByRole('button',{name:'Lectura',exact:true}).click();await page.getByRole('button',{name:'Uso de IA',exact:true}).click();await claude.getByRole('progressbar').nth(1).waitFor();
  mkdirSync('../../.local',{recursive:true});await page.screenshot({path:'../../.local/ai-usage-preview.png',fullPage:true});
  assert.deepEqual(errors,[]);
  console.log('PASS: account quota windows, remaining percentages, missing/stale/expired data, connection/disconnection, provider link, error recovery, no localStorage quota persistence and responsive layout (mocked IPC).');
}finally{await browser.close();}
