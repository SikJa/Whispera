import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0}});await server?.listen();
const browser=await launchSilentBrowser();
function luminance(hex){const rgb=hex.match(/[0-9a-f]{2}/gi).map(x=>parseInt(x,16)/255).map(x=>x<=.04045?x/12.92:((x+.055)/1.055)**2.4);return rgb[0]*.2126+rgb[1]*.7152+rgb[2]*.0722;}
function contrast(a,b){const [x,y]=[luminance(a),luminance(b)].sort((a,b)=>b-a);return (x+.05)/(y+.05);}
try {
 const page=await browser.newPage({viewport:{width:1000,height:820}});await page.addInitScript(installNativeMock,{});
 const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
 await page.goto(base);await page.getByRole('button',{name:'Apariencia',exact:true}).click();
 const choices=['Ciruela','Grafito','Azul noche','Bosque','Pizarra','Arena'];
 assert.equal(await page.locator('.app-theme-options button').count(),6);
 for(const name of choices){
   await page.getByRole('button',{name,exact:true}).click();
   await page.waitForTimeout(650);
   assert.equal(await page.getByRole('button',{name,exact:true}).getAttribute('aria-pressed'),'true');
   const colors=await page.evaluate(()=>{const style=getComputedStyle(document.documentElement);return Object.fromEntries(['background','surface','card','ink','muted','accent'].map(key=>[key,style.getPropertyValue(`--app-${key}`).trim()]));});
   for(const surface of ['background','surface','card']) {
     assert.ok(contrast(colors.ink,colors[surface])>=4.5,`${name}: text on ${surface}`);
     assert.ok(contrast(colors.muted,colors[surface])>=4.5,`${name}: secondary text on ${surface}`);
   }
   assert.ok(contrast(colors.accent,colors.background)>=4.5);
   assert.ok(await page.evaluate(()=>window.calls.some(c=>c.command==='save_settings'&&c.args.settings.appTheme===document.documentElement.dataset.appTheme)));
 }
 await page.screenshot({path:'../../.local/themes-arena.png'});
 await page.setViewportSize({width:620,height:760});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true,'theme picker must not create horizontal scrolling');
 await page.getByRole('button',{name:'Transcripción',exact:true}).click();
 assert.equal(await page.evaluate(()=>document.documentElement.dataset.appTheme),'sand');
 console.log('PASS: six coordinated themes, autosave, text/secondary/accent contrast and compact layout. Native settings IPC mocked.');
} finally {await browser.close();await server?.close();}
