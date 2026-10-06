import {createServer} from 'vite';
import {spawn} from 'node:child_process';
import {mkdirSync} from 'node:fs';

mkdirSync('../../.local', {recursive:true});
const server = await createServer({server:{host:'127.0.0.1',port:0,strictPort:false}});
await server.listen();
const base = `http://127.0.0.1:${server.httpServer.address().port}`;
const suites = ['screen','selection-ready','adjustable-frame','editor','editor-races','editor-paint','line-toolbar','capture-controls','history','hotkeys','setup','native-library','library-settings','library-media'];
const failures=[];
try {
  for (const name of suites) {
    const code=await new Promise((resolve,reject)=>{
      const child=spawn(process.execPath,[`verify-${name}.mjs`],{stdio:'inherit',windowsHide:true,env:{...process.env,WHISPERA_TEST_URL:base}});
      child.once('error',reject);
      const timer=setTimeout(()=>child.kill(),120000);
      child.once('exit',code=>{clearTimeout(timer);resolve(code);});
    });
    if(code!==0)failures.push(name);
  }
} finally { await server.close(); }
if(failures.length)throw Error(`Failed suites: ${failures.join(', ')}`);
console.log(`PASS: ${suites.length} silent headless suites. Native IPC is mocked.`);
