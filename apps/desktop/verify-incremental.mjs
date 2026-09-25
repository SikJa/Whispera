import { chromium } from 'playwright';
import { createServer } from 'vite';
import assert from 'node:assert/strict';

const server = await createServer({ server: {host:'127.0.0.1',port:0,open:false} });
await server.listen();
const browser = await chromium.launch({channel:'msedge',headless:true});
try {
  const page = await browser.newPage({viewport:{width:900,height:720}});
  const errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto(server.resolvedUrls.local[0]);
  const toggle=page.getByRole('switch',{name:'Transcripción anticipada'});
  const trim=page.getByRole('switch',{name:'Recortar silencios'});
  await toggle.waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'true');
  assert.equal(await trim.isDisabled(),true);
  await toggle.click();
  assert.equal(await trim.isDisabled(),false);
  await page.getByRole('button',{name:'Guardar',exact:true}).click();
  await page.getByRole('status').filter({hasText:'guardado'}).waitFor();
  await page.reload();
  await toggle.waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'false');
  await toggle.click();
  await page.getByRole('button',{name:'Guardar',exact:true}).click();
  await page.getByRole('status').filter({hasText:'guardado'}).waitFor();
  await page.reload();
  await toggle.waitFor();
  assert.equal(await toggle.getAttribute('aria-checked'),'true');
  for(const viewport of [{width:900,height:720},{width:390,height:844}]) {
    await page.setViewportSize(viewport);
    await toggle.scrollIntoViewIfNeeded();
    assert(await toggle.isVisible());
    assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
  }
  assert.deepEqual(errors,[]);
  console.log('PASS: default enabled, opt-out/in persistence, trim interlock, desktop/mobile, no page errors. No native recording or API requests.');
} finally {
  await browser.close();
  await server.close();
}
