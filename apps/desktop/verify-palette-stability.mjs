import assert from 'node:assert/strict';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
import {realpathSync} from 'node:fs';
const server=process.env.WHISPERA_TEST_URL?null:await createServer({server:{host:'127.0.0.1',port:0,fs:{allow:[process.cwd(),realpathSync('node_modules')]}}});
await server?.listen();
const browser=await launchSilentBrowser();
let drags=0;
try {
  const page=await browser.newPage();
  await page.addInitScript(installNativeMock,{});
  const base=process.env.WHISPERA_TEST_URL||`http://127.0.0.1:${server.httpServer.address().port}`;
  for(const width of [620,900,1200]) {
    await page.setViewportSize({width,height:720});
    await page.goto(base);
    await page.getByRole('button',{name:'Apariencia',exact:true}).click();
    await page.evaluate(()=>document.fonts.ready);
    for(const label of ['Elegir color de fondo','Elegir color']) {
      await page.getByRole('button',{name:label,exact:true}).click();
      await page.getByRole('button',{name:'Personalizado',exact:true}).click();
      const area=page.locator('.palette-area');
      const before=await area.boundingBox();
      const main=page.locator('.desktop-main');
      const scroll=await main.evaluate(el=>el.scrollTop);
      const stable=async()=>{
        const after=await area.boundingBox();
        assert.ok(Math.abs(after.x-before.x)<=1&&Math.abs(after.y-before.y)<=1,`${width}/${label}: palette moved during selection: ${JSON.stringify({before,after})}`);
        assert.ok(Math.abs(await main.evaluate(el=>el.scrollTop)-scroll)<=1,`${width}/${label}: content scrolled while selecting a color`);
      };
      for(let repeat=0;repeat<3;repeat++) {
        await page.mouse.move(before.x+before.width*.6,before.y+before.height*.4);
        await page.mouse.down();
        try {
          for(const [x,y] of [[.95,.9],[.1,.05],[.8,.6],[0,1],[1,0]]) {
            await page.mouse.move(before.x+before.width*x,before.y+before.height*y,{steps:8});
            await page.waitForTimeout(40);await stable();
          }
        } finally {await page.mouse.up();}
        drags++;
        const track=await page.locator('.palette-track').boundingBox();
        await page.mouse.move(track.x+track.width*.2,track.y+track.height/2);
        await page.mouse.down();
        try {
          for(const x of [.9,.05,.6]) {
            await page.mouse.move(track.x+track.width*x,track.y+track.height/2,{steps:8});
            await page.waitForTimeout(40);await stable();
          }
        } finally {await page.mouse.up();}
        drags++;
      }
      for(const color of ['#000000','#F3F3F3','#50B1FD','#FFFFFF','#000000','#FF0000']) {
        const field=page.getByRole('textbox',{name:'Código HEX'});
        await field.fill(color);await field.press('Enter');
        await page.waitForTimeout(40);await stable();
      }
      const input=area.getByRole('slider').first();
      await input.focus();await input.press('ArrowRight');await stable();
      if(label==='Elegir color de fondo') {
        await page.waitForFunction(()=>window.calls.some(c=>c.command==='save_settings'&&c.args.settings.appBackground?.toLowerCase()==='#ff0000'));
      }
      await page.getByRole('button',{name:'Cerrar paleta',exact:true}).click();
      console.log(`PASS: ${width}px / ${label}: area, hue, presets-as-HEX, keyboard and unchanged scroll.`);
    }
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true,'Palette controls must fit compact windows');
  }
  console.log(`PASS: ${drags} repeated area/hue drags across 3 window sizes and both palettes. Native settings IPC mocked.`);
} finally {await browser.close();await server?.close();}
