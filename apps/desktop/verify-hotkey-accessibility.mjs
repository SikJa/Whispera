import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});await server?.listen();
const browser=await launchSilentBrowser();
try {
 const page=await browser.newPage();
 await page.goto(process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`,{timeout:90000});
 const results=await page.evaluate(async()=>{
  const {eventHotkey}=await import('/src/hotkey-capture.ts');
  return [
   {key:'F8',code:'',ctrlKey:true,altKey:true},
   {key:'r',code:'Unidentified',ctrlKey:true,shiftKey:true},
   {key:'z',code:'KeyY',altKey:true},
   {key:'?',code:''},
   {key:'Control',code:''},
   {key:'F8',code:'',repeat:true},
   {key:'F8',code:'',isComposing:true},
  ].map(x=>eventHotkey(new KeyboardEvent('keydown',x))??null);
 });
 assert.deepEqual(results,['Control+Alt+F8','Control+Shift+KeyR','Alt+KeyY',null,null,null,null]);
 console.log('PASS: known virtual-key fallback, physical layout precedence, unknown punctuation/modifiers/repeats/composition rejected.');
} finally {await browser.close();await server?.close();}
