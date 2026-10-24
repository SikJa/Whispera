import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  for(const [view,kind,label] of [['screen-tools','video','herramientas'],['screen-hud','video','controles de video'],['screen-hud','image','acciones de captura']]) {
    const page=await browser.newPage({viewport:{width:1100,height:780}}),errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(installNativeMock,{kind});
    await page.goto(`${process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190'}/overlay.html?view=${view}`);
    const grip=page.getByRole('button',{name:`Mover ${label}`,exact:true});await grip.waitFor();
    const b=await grip.boundingBox();
    await page.mouse.move(b.x+b.width/2,b.y+b.height/2);await page.mouse.down();
    await page.mouse.move(b.x+b.width/2+140,b.y+b.height/2+70,{steps:5});await page.mouse.up();
    await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_overlay_customize'&&c.args.action==='end'));
    const moves=await page.evaluate(()=>window.calls.filter(c=>c.command==='screen_overlay_customize'&&c.args.action==='move'));
    assert.ok(moves.length>0);assert.equal(moves.at(-1).args.x,140);assert.equal(moves.at(-1).args.y,70);
    await page.getByRole('button',{name:`Ampliar ${label}`,exact:true}).click();
    await page.waitForFunction(()=>window.captureDock.zoom===1.1);
    await page.getByRole('button',{name:`Reducir ${label}`,exact:true}).click();
    await page.waitForFunction(()=>window.captureDock.zoom===1);
    await page.getByRole('button',{name:`Contraer ${label}`,exact:true}).click();
    await page.getByRole('button',{name:`Expandir ${label}`,exact:true}).waitFor();
    await grip.waitFor();
    if(kind==='video'&&view==='screen-hud')assert.equal(await page.getByRole('button',{name:'Detener video'}).count(),1);
    if(kind==='image')assert.equal(await page.getByRole('button',{name:'Copiar',exact:true}).count(),1);
    await page.getByRole('button',{name:`Expandir ${label}`,exact:true}).click();
    await page.getByRole('button',{name:`Contraer ${label}`,exact:true}).waitFor();
    const measured=await page.evaluate(()=>window.calls.filter(c=>c.command==='screen_overlay_customize'&&c.args.action==='size').map(c=>c.args));
    assert.ok(measured.length>0&&measured.every(x=>Number.isFinite(x.width)&&Number.isFinite(x.height)));
    assert.deepEqual(errors,[]);await page.close();
  }
  console.log('PASS: independent dock dragging, collapse, scale, sizing and essential actions (mocked IPC).');
}finally{await browser.close();}
