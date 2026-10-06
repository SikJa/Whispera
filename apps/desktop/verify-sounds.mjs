import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
const server = process.env.WHISPERA_TEST_URL ? null : await createServer({server:{host:'127.0.0.1',port:0}});
await server?.listen();
const browser = await launchSilentBrowser();
try {
  const page = await browser.newPage();
  await page.addInitScript(() => {
    window.samples = [];
    const original = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function(...args) {
      window.samples.push(this);
      return original.apply(this, args);
    };
  });
  await page.goto(process.env.WHISPERA_TEST_URL || `http://127.0.0.1:${server.httpServer.address().port}`);
  await page.getByRole('button',{name:'Sonidos',exact:true}).click();
  await page.locator('#sound-theme').selectOption('gota');
  await page.waitForFunction(() => window.samples.length === 2);
  assert.deepEqual(await page.evaluate(() => window.samples.map(s=>new URL(s.src).pathname)), ['/sound-lab/gota-start.wav','/sound-lab/gota-stop.wav']);
  assert.equal(await page.evaluate(() => localStorage.getItem('whispera-v2-preview')), null, 'audition must not save settings');
  await page.waitForFunction(() => window.samples.every(s=>s.paused));
  await page.locator('#sound-theme').selectOption('madera');
  await page.locator('#sound-theme').selectOption('seda');
  assert.equal(await page.evaluate(() => window.samples.at(-2).paused), true, 'new audition stops old one');
  await page.getByRole('button',{name:'Transcripción',exact:true}).click();
  const count = await page.evaluate(() => window.samples.length);
  await page.waitForTimeout(1200);
  assert.equal(await page.evaluate(() => window.samples.length), count, 'leaving sounds cancels delayed playback');
  assert.equal(await page.evaluate(() => window.samples.every(s=>s.paused)), true);
  await page.getByRole('button',{name:'Sonidos',exact:true}).click();
  await page.getByRole('button',{name:'Guardar',exact:true}).click();
  await page.waitForFunction(() => JSON.parse(localStorage.getItem('whispera-v2-preview') || '{}').settings?.soundTheme === 'seda');
  await page.reload();
  await page.getByRole('button',{name:'Sonidos',exact:true}).click();
  assert.equal(await page.locator('#sound-theme').inputValue(), 'seda');
  console.log('PASS: real muted audio preview, explicit saving, interruption and navigation cleanup.');
} finally { await browser.close(); await server?.close(); }
