import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
 const page=await browser.newPage();await page.addInitScript(installNativeMock,{});
 await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/');
 await page.getByRole('button',{name:'Actualizaciones',exact:true}).click();
 await page.getByText('Versión experimental de danieluxury88.',{exact:false}).waitFor();
 assert.equal(await page.getByRole('button',{name:'Buscar actualizaciones',exact:true}).count(),0);
 assert.ok(!await page.evaluate(()=>window.calls.some(c=>['updater_check','updater_install'].includes(c.command))));
 console.log('PASS: fork updater page offers no upstream check/install actions; no update request issued.');
}finally{await browser.close();}
