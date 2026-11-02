import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile, stat} from 'node:fs/promises';
import {createServer} from 'node:http';
import {dirname, extname, isAbsolute, relative, resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArgs} from 'node:util';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';

const {values} = parseArgs({options:{
  help:{type:'boolean'}, samples:{type:'string',default:'20'}, warmups:{type:'string',default:'2'},
  'ipc-delay-ms':{type:'string',default:'0'}, root:{type:'string',default:'dist'},
  only:{type:'string'}, raw:{type:'boolean'},
}});
if(values.help) {
  console.log(`Usage: node verify-startup-latency.mjs [--samples 20] [--warmups 2]
  [--ipc-delay-ms 80] [--root dist] [--only video-selector,clipboard] [--raw]

Read-only static build server on 127.0.0.1:0; never uses preview5190 or Tauri.
Headless, audio disabled, external requests blocked. No builds or report files.
Cold = new browser context/navigation, NOT cold OS/native process/disk cache.
Warm = close/reset and reopen the SAME document with a new simulated session.
Nonzero --ipc-delay-ms runs both zero-delay and delayed simulated IPC profiles.
Results: nearest-rank p50/p95, request coverage, simulated IPC coverage and raw
samples with --raw. Coverage is wall time, NOT CPU time or native latency.
The compiled artifacts must already exist. Rebuild separately when authorized.`);
  process.exit(0);
}
assert.ok(!process.env.WHISPERA_TEST_BROWSER || process.env.WHISPERA_TEST_BROWSER === 'chromium', 'This benchmark requires Chromium resource timing.');
const integer = (name,min,max) => {
  const n = Number(values[name]);
  assert.ok(Number.isInteger(n) && n >= min && n <= max, `Invalid --${name}: expected ${min}..${max}`);
  return n;
};
const samples = integer('samples',1,200), warmups = integer('warmups',0,20), delay = integer('ipc-delay-ms',0,1000);
const here = dirname(fileURLToPath(import.meta.url)), root = resolve(here,values.root);
const surfaces = [
  {name:'image-ink',view:'screen-ink',kind:'image',selector:'canvas'},
  {name:'image-tools',view:'screen-tools',kind:'image',selector:'[aria-label="Herramientas de captura"]'},
  {name:'image-actions',view:'screen-hud',kind:'image',selector:'[aria-label="Acciones de captura"]'},
  {name:'video-selector',view:'screen-select',kind:'video',selector:'.screen-selection'},
  {name:'video-tools',view:'screen-tools',kind:'video',selector:'[aria-label="Herramientas de captura"]'},
  {name:'video-hud',view:'screen-hud',kind:'video',selector:'[aria-label="Controles de video"]'},
  {name:'clipboard',kind:'image',selector:'.blade'},
];
const requested = values.only?.split(',');
if(requested) assert.ok(requested.every(name=>surfaces.some(s=>s.name===name)), 'Unknown --only surface');
const selected = surfaces.filter(s=>!requested || requested.includes(s.name));
const entries = [...new Set(selected.map(s=>s.name==='clipboard'?'library/browser-preview.html':'overlay.html'))];
for(const entry of entries) await stat(resolve(root,entry));

const assets = new Map(), failures = [], rows = [], blockedNativeAssets = new Set();
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const mime = {'.html':'text/html','.js':'text/javascript','.css':'text/css','.woff':'font/woff','.woff2':'font/woff2','.png':'image/png','.svg':'image/svg+xml','.webp':'image/webp'};
const server = createServer(async(req,res)=>{
  try {
    const pathname = decodeURIComponent(new URL(req.url,'http://localhost').pathname);
    if(pathname==='/favicon.ico'){res.writeHead(204).end();return;}
    const file = resolve(root,`.${pathname}`), rel = relative(root,file);
    if(rel==='..' || rel.startsWith(`..${sep}`) || isAbsolute(rel)){res.writeHead(403).end();return;}
    if(!assets.has(file)) {
      const bytes = await readFile(file), info = await stat(file);
      assets.set(file,{bytes,sha256:hash(bytes),mtime:info.mtime.toISOString()});
    }
    const asset = assets.get(file);
    res.writeHead(200,{'Content-Type':mime[extname(file)]||'application/octet-stream','Cache-Control':'no-store','Content-Length':asset.bytes.length});
    res.end(asset.bytes);
  } catch {res.writeHead(404).end();}
});

function installLatencyHarness({surface,ipcDelayMs}) {
  // This only wraps the existing in-page mock; no real IPC transport is retained.
  const original = window.__TAURI_INTERNALS__.invoke;
  const q = window.__startupLatency = {epoch:0,start:0,spans:[],delays:[],finished:null,handshake:null,hidden:0};
  const settings = {tutorialCompleted:true,hoverActivation:false,language:'es',stickPosition:'left',soundEffects:false,reduceMotion:false};
  const items = Array.from({length:250},(_,i)=>({id:`latency-${i}`,capturedAt:Date.now()-i,hitCount:1,pinned:false,data:{kind:'text',text:`Synthetic clipboard item ${i}`,isUrl:false}}));
  const reads = new Set(['screen_editor_context','screen_editor_image','screen_editor_background','screen_editor_feedback_get','screen_appearance','screen_status','screen_selection_kind','screen_monitors','screen_overlay_layout','screen_overlay_customize','screen_editor_ready','screen_overlay_ready','library_state','recorder_region','library_window']);
  const pixel = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=';
  window.__TAURI_INTERNALS__.convertFileSrc = () => pixel;
  HTMLMediaElement.prototype.play = () => Promise.resolve();
  window.AudioContext = undefined;
  window.webkitAudioContext = undefined;
  if(navigator.mediaDevices) {
    navigator.mediaDevices.getUserMedia = async()=>{throw Error('Hardware access disabled by latency test');};
    navigator.mediaDevices.getDisplayMedia = async()=>{throw Error('Hardware access disabled by latency test');};
  }
  window.videoStatus.phase = surface.view==='screen-select'?'selecting':'recording';
  window.editorContext = {...window.editorContext,rect:{x:0,y:0,width:640,height:480},monitor_width:640,monitor_height:480,source_label:'screen-select-0'};
  function interactive() {
    const element = document.querySelector(surface.selector);
    if(!element || !element.getBoundingClientRect().width) return false;
    if(surface.name==='clipboard') {
      const rect=element.getBoundingClientRect();
      return document.querySelectorAll('.item-main').length>0 && rect.x>=-1 && Number(getComputedStyle(element).opacity)>=0.95;
    }
    if(surface.name==='video-hud') return document.querySelector('button[aria-label="Detener video"]')?.disabled===false;
    if(surface.name==='video-selector') return Boolean(element.querySelector('.screen-selection-help'));
    return surface.view==='screen-ink' || Boolean(element.querySelector('button:not(:disabled)'));
  }
  function markReady(epoch) {
    if(epoch!==q.epoch || q.handshake!==null) return;
    q.handshake = performance.now();
    const check = () => {
      if(epoch!==q.epoch || q.finished) return;
      if(!interactive()){requestAnimationFrame(check);return;}
      const interactiveAt=performance.now();
      requestAnimationFrame(()=>{
        if(epoch===q.epoch) q.finished={start:q.start,handshake:q.handshake,interactiveAt,end:performance.now()};
      });
    };
    requestAnimationFrame(check);
  }
  window.__TAURI_INTERNALS__.invoke = async(command,args={}) => {
    const start=performance.now(), epoch=q.epoch;
    try {
      if(ipcDelayMs && reads.has(command)) {
        const waiting=performance.now();
        await new Promise(resolve=>setTimeout(resolve,ipcDelayMs));
        q.delays.push([waiting,performance.now()]);
      }
      let result;
      if(command==='library_state') result=args.revision==='latency-fixture'?{unchanged:true}:{items,settings,revision:'latency-fixture',version:'simulated IPC',isStoreBuild:false};
      else if(command==='library_cursor') result=null;
      else if(command==='library_window' || command==='recorder_region' || command==='screen_editor_ready' || command==='screen_overlay_ready') result=true;
      else if(command==='screen_editor_background' || command==='screen_editor_image') {
        // Synthetic 640x480 opaque BMP, not desktop pixels or a capture benchmark.
        const bytes=new Uint8Array(54+640*480*4), header=new DataView(bytes.buffer);
        bytes.fill(238,54);bytes[0]=66;bytes[1]=77;
        header.setUint32(2,bytes.length,true);header.setUint32(10,54,true);header.setUint32(14,40,true);
        header.setInt32(18,640,true);header.setInt32(22,-480,true);header.setUint16(26,1,true);header.setUint16(28,32,true);
        result=bytes.buffer;
      } else result=await original(command,args);
      if(command==='screen_editor_ready' || command==='screen_overlay_ready' || (command==='library_window' && args.operation==='show')) markReady(epoch);
      if(command==='library_window' && args.operation==='hide') q.hidden++;
      return result;
    } finally {q.spans.push({command,start,end:performance.now()});}
  };
  window.__beginStartupLatency = () => {
    q.epoch++;q.start=performance.now();q.handshake=null;q.finished=null;
    if(surface.name==='clipboard') window.emitNative('library-open',undefined);
    else if(surface.view==='screen-select') {
      window.videoStatus.phase='selecting';window.emitNative('screen-reset','video');
    } else {
      window.editorContext={...window.editorContext,id:`latency-session-${q.epoch}`};
      window.emitNative('screen-editor-reset',window.editorContext);
    }
  };
  window.__collectStartupLatency = () => {
    const {start,end,handshake,interactiveAt}=q.finished;
    const fetches=performance.getEntriesByType('resource').concat(performance.getEntriesByType('navigation'))
      .filter(e=>/^https?:/.test(e.name) && e.fetchStart<end && e.responseEnd>start);
    const requests=fetches.map(e=>[Math.max(start,e.fetchStart),Math.min(end,e.responseEnd)]);
    const ipc=q.spans.map(s=>[Math.max(start,s.start),Math.min(end,s.end)]).filter(([a,b])=>b>a);
    const union=spans=>{
      const ordered=spans.filter(([a,b])=>b>a).sort((a,b)=>a[0]-b[0]);let total=0,right=-Infinity;
      for(const [a,b] of ordered){total+=Math.max(0,b-Math.max(a,right));right=Math.max(right,b);}return total;
    };
    const fetch=union(requests), simulated=union(ipc), both=union([...requests,...ipc]), total=end-start;
    const commands={};
    for(const s of q.spans.filter(s=>s.start>=start && s.start<end)) {
      const c=commands[s.command]??={calls:0,sumMs:0};c.calls++;c.sumMs+=Math.min(end,s.end)-s.start;
    }
    return {totalMs:total,handshakeMs:handshake-start,interactiveMs:interactiveAt-start,
      fetchCoverageMs:fetch,simulatedIpcCoverageMs:simulated,
      injectedDelayCoverageMs:union(q.delays.map(([a,b])=>[Math.max(start,a),Math.min(end,b)])),
      fetchIpcOverlapMs:fetch+simulated-both,browserOtherMs:Math.max(0,total-both),
      transferredBytes:fetches.reduce((n,e)=>n+e.transferSize,0),
      scriptBytes:fetches.filter(e=>e.initiatorType==='script' || /\.js(?:\?|$)/.test(e.name)).reduce((n,e)=>n+e.decodedBodySize,0),commands};
  };
}

const percentile = (numbers,p) => [...numbers].sort((a,b)=>a-b)[Math.max(0,Math.ceil(numbers.length*p)-1)];
const rounded = n => Math.round(n*100)/100;
let browser;
try {
  await new Promise((done,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',done);});
  const origin=`http://127.0.0.1:${server.address().port}`;
  assert.notEqual(server.address().port,5190);
  browser=await launchSilentBrowser();
  for(const ipcDelayMs of delay?[0,delay]:[0]) for(const surface of selected) {
    console.error(`Measuring ${surface.name}: simulated IPC=${ipcDelayMs} ms, ${samples} cold/warm pairs + ${warmups} warmup pairs`);
    for(let round=0;round<samples+warmups;round++) {
      const page=await browser.newPage({viewport:{width:1280,height:800}}), context=page.context();
      const errors=[], external=[], requestErrors=[];
      page.on('pageerror',e=>errors.push(e.message));
      page.on('response',res=>{if(res.status()>=400)requestErrors.push(`${res.status()} ${res.url()}`);});
      await context.route('**/*',route=>{
        const url=new URL(route.request().url());
        if(url.origin===origin) return route.continue();
        // The upstream idle emoji warmer uses a native protocol unavailable in Chromium.
        if(url.protocol==='edgelocal:' && url.hostname==='emoji') {
          blockedNativeAssets.add(url.href);return route.abort();
        }
        external.push(url.href);return route.abort();
      });
      await page.addInitScript(installNativeMock,{kind:surface.kind,windowLabel:surface.name==='clipboard'?'library':undefined,width:640,height:480});
      await page.addInitScript(installLatencyHarness,{surface,ipcDelayMs});
      let phase='navigation';
      try {
        const url=surface.name==='clipboard'?`${origin}/library/browser-preview.html`:`${origin}/overlay.html?view=${surface.view}`;
        await page.goto(url,{waitUntil:'commit',timeout:15000});
        for(const temperature of ['cold','warm']) {
          phase=temperature;
          if(temperature==='warm') {
            if(surface.name==='clipboard') {
              const hidden=await page.evaluate(()=>{const before=window.__startupLatency.hidden;window.emitNative('library-close',undefined);return before;});
              await page.waitForFunction(n=>window.__startupLatency.hidden>n,hidden,{polling:20,timeout:15000});
              await page.waitForTimeout(450);
            } else {
              await page.evaluate(s=>window.emitNative(s.view==='screen-select'?'screen-hide':'screen-editor-reset',null),surface);
              await page.locator(surface.selector).waitFor({state:'detached',timeout:15000});
            }
            await page.evaluate(()=>window.__beginStartupLatency());
          }
          await page.waitForFunction(()=>window.__startupLatency.finished!==null,null,{polling:20,timeout:15000});
          // Inspect settled DOM only after readiness; networkidle is not the latency endpoint.
          await page.waitForLoadState('networkidle',{timeout:15000});
          assert.ok(await page.locator(surface.selector).first().isVisible(), `${surface.name}: surface missing`);
          assert.equal(await page.getByRole('alert').count(),0, `${surface.name}: alert present`);
          const sample=await page.evaluate(()=>window.__collectStartupLatency());
          assert.deepEqual(errors,[],`${surface.name}: browser errors`);
          assert.deepEqual(requestErrors,[],`${surface.name}: broken assets`);
          assert.deepEqual(external,[],`${surface.name}: external requests attempted`);
          if(round>=warmups) rows.push({surface:surface.name,temperature,ipcDelayMs,round:round-warmups,...sample});
        }
      } catch(error) {
        const diagnostics=await page.evaluate(()=>({handshake:window.__startupLatency?.handshake,
          calls:window.calls?.map(c=>c.command),dom:document.getElementById('root')?.innerHTML.slice(0,600)})).catch(()=>null);
        failures.push({surface:surface.name,phase,ipcDelayMs,round,message:String(error),errors,requestErrors,external,diagnostics});
        console.error(JSON.stringify(failures.at(-1)));
        throw error;
      } finally {await page.close();}
    }
  }
  const summary=[];
  for(const ipcDelayMs of delay?[0,delay]:[0]) for(const surface of selected) for(const temperature of ['cold','warm']) {
    const group=rows.filter(r=>r.surface===surface.name && r.temperature===temperature && r.ipcDelayMs===ipcDelayMs);
    const metrics={};
    for(const key of ['totalMs','handshakeMs','interactiveMs','fetchCoverageMs','simulatedIpcCoverageMs','injectedDelayCoverageMs','fetchIpcOverlapMs','browserOtherMs','transferredBytes','scriptBytes']) {
      metrics[key]={p50:rounded(percentile(group.map(r=>r[key]),0.5)),p95:rounded(percentile(group.map(r=>r[key]),0.95))};
    }
    summary.push({surface:surface.name,temperature,ipcDelayMs,n:group.length,metrics});
  }
  const artifactList=[];
  for(const [file,asset] of assets) {
    assert.equal(hash(await readFile(file)),asset.sha256,`Served artifact changed during benchmark: ${file}`);
    artifactList.push({path:relative(root,file),sha256:asset.sha256,bytes:asset.bytes.length,mtime:asset.mtime});
  }
  const report={scope:'Headless frontend only; simulated IPC; NOT native Windows/OS clipboard/capture/encoder/WebView2 latency',
    cold:'New isolated browser context/navigation per pair; browser process and OS disk cache NOT cold',
    warm:'Same document, synthetic close/reset + new session/open; no navigation',
    endpoint:'Mock ready/show response + usable DOM + next animation frame; paint opportunity, not compositor presentation',
    coverage:'Union of in-flight HTTP fetch intervals and mock IPC intervals, clipped to endpoint. They may overlap. browserOtherMs = total minus their union, NOT CPU time. Per-command sums may overlap; never add them to total.',
    fixtures:{bitmap:'synthetic 640x480 opaque BMP',clipboard:'250 synthetic text entries; cached unchanged state on reopen',audio:'disabled, no playback or devices',
      nativeEmojiWarmup:'Native-protocol emoji requests blocked; their native asset load cost is NOT measured',blockedNativeEmojiAssets:blockedNativeAssets.size},
    buildRoot:root,browser:browser.version(),samplesPerGroup:samples,warmupsPerSurface:warmups,
    warning:samples<20?'Fewer than 20 samples: p95 is exploratory, not a release gate.':'Local distribution only, not a hardware guarantee.',
    artifacts:artifactList,summary,...(values.raw?{samples:rows}:{}),failures};
  console.table(summary.map(r=>({surface:r.surface,mode:r.temperature,delay:r.ipcDelayMs,n:r.n,
    p50:r.metrics.totalMs.p50,p95:r.metrics.totalMs.p95,ipcP50:r.metrics.simulatedIpcCoverageMs.p50,
    fetchP50:r.metrics.fetchCoverageMs.p50,otherP50:r.metrics.browserOtherMs.p50})));
  console.log(`REPORT ${JSON.stringify(report)}`);
} finally {
  await browser?.close();
  server.closeAllConnections();
  await new Promise(done=>server.close(done));
}
