import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';

const exports={};
vm.runInNewContext(ts.transpileModule(readFileSync(new URL('./vendor/edge-drop/browser-preview/window-lifecycle.ts',import.meta.url),'utf8'),{
  compilerOptions:{module:ts.ModuleKind.CommonJS},
}).outputText,{exports});
const deferred=()=>{let resolve;const promise=new Promise(r=>{resolve=r;});return {promise,resolve};};
const rect={x:0,y:0,width:280,height:700};
function harness() {
  const calls=[];
  let region,show,fail=false,rects=[rect];
  const panel=exports.libraryWindow(async(command,args)=>{
    calls.push({command,...args});
    if(command==='recorder_region'){if(fail)throw Error('region failed');await region?.promise;}
    if(args.operation==='show')await show?.promise;
  },()=>rects);
  return {panel,calls,region:v=>region=v,show:v=>show=v,fail:v=>fail=v,rects:v=>rects=v};
}
const closed=harness();await closed.panel.sync();assert.equal(closed.calls.length,0,'hidden DOM must never reopen a closed panel');
const delayed=harness(),region=deferred();delayed.region(region);
const opening=delayed.panel.open();await delayed.panel.close();region.resolve();await opening;
assert.equal(delayed.calls.filter(c=>c.operation==='show').length,0,'a late region must not resurrect a closed panel');
const dispatched=harness(),show=deferred();dispatched.show(show);
const showing=dispatched.panel.open();await new Promise(resolve=>setImmediate(resolve));
await dispatched.panel.close();show.resolve();await showing;
assert.equal(dispatched.calls.at(-1).operation,'hide','close must win over in-flight native show');
const failed=harness();failed.fail(true);await assert.rejects(failed.panel.open(),/region failed/);
assert.equal(failed.calls.at(-1).operation,'hide');assert.equal(failed.calls.some(c=>c.operation==='show'),false);
failed.fail(false);await failed.panel.open();assert.equal(failed.calls.at(-1).operation,'show');
const normal=harness();await normal.panel.open();const count=normal.calls.length;await normal.panel.sync();assert.equal(normal.calls.length,count);
await normal.panel.close();await normal.panel.sync();assert.equal(normal.calls.at(-1).operation,'hide');
await normal.panel.open();assert.equal(normal.calls.at(-1).operation,'show');
normal.rects([]);await normal.panel.sync();assert.equal(normal.calls.at(-1).operation,'hide','empty geometry must not keep an interactive surface');
console.log('PASS: closed DOM, late region/show, native region failure, explicit reopen and empty hit-region safety. IPC mocked.');
