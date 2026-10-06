import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {PNG} from 'pngjs';
import {installNativeMock} from './tests/native-mock.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage({viewport:{width:640,height:480}}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(installNativeMock,{kind:'image'});
  await page.goto((process.env.WHISPERA_TEST_URL || 'http://127.0.0.1:5190') + '/overlay.html?view=screen-ink');
  await page.getByLabel('Editar captura').waitFor();
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready')&&!window.feedback.busy);
  for(const [i,tool] of ['rectangle','ellipse','triangle','diamond','hexagon','star'].entries()){
    await page.evaluate(tool=>window.emitNative('screen-editor-action',{id:'test-session',action:{action:'tool',value:tool}}),tool);
    await page.waitForFunction(tool=>window.feedback.tool===tool,tool);
    await page.mouse.move(35+i*90,60);await page.mouse.down();await page.mouse.move(100+i*90,150);await page.mouse.up();
    await page.waitForFunction(n=>window.feedback.count===n,i+1);
  }
  await page.keyboard.press('Control+c');await page.waitForFunction(()=>window.exports.length===1);
  const png=PNG.sync.read(Buffer.from(await page.evaluate(()=>window.exports[0].bytes)));
  for(let i=0;i<6;i++){
    let red=0;for(let y=55;y<156;y++)for(let x=30+i*90;x<105+i*90;x++){const p=(y*png.width+x)*4;if(png.data[p]>240&&png.data[p+1]<160)red++;}
    assert.ok(red>100,`shape ${i} appears in PNG (${red} pixels)`);
  }
  await page.keyboard.press('Control+z');await page.waitForFunction(()=>window.feedback.count===5);await page.keyboard.press('Control+y');await page.waitForFunction(()=>window.feedback.count===6);
  await page.goto((process.env.WHISPERA_TEST_URL || 'http://127.0.0.1:5190') + '/overlay.html?view=screen-tools');await page.setViewportSize({width:350,height:442});
  await page.getByRole('button',{name:'Formas',exact:true}).click();
  for(const name of ['Rectángulo','Círculo / elipse','Triángulo','Rombo','Hexágono','Estrella'])assert.equal(await page.getByRole('button',{name,exact:true}).count(),1);
  await page.screenshot({path:'../../.local/line-shapes.png'});
  await page.getByRole('button',{name:'Estrella',exact:true}).click();await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_action'&&c.args.action.value==='star'));
  await page.getByRole('dialog').waitFor({state:'detached'});
  await page.getByRole('button',{name:'Más herramientas',exact:true}).click();
  assert.equal(await page.getByRole('dialog').getByRole('button',{name:'Lápiz'}).count(),0,'no duplicate drawing tools');
  assert.equal(await page.getByRole('dialog').getByRole('button',{name:'Eliminar selección'}).count(),1);
  await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'detached'});
  await page.getByRole('button',{name:'Elegir color'}).click();
  await page.getByRole('button',{name:'Personalizado',exact:true}).click();
  await page.getByRole('textbox',{name:'Código HEX'}).fill('#12ab34');await page.getByRole('textbox',{name:'Código HEX'}).press('Enter');
  await page.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_action'&&c.args.action.action==='color'&&c.args.action.value==='#12AB34'));
  await page.screenshot({path:'../../.local/line-custom-color.png'});
  const area=await page.getByRole('dialog').boundingBox();assert.ok(area.x>=0&&area.y>=0&&area.x+area.width<=350&&area.y+area.height<=442,'custom palette fits without scrolling');
  await page.getByRole('button',{name:'Cerrar paleta'}).click();await page.getByRole('dialog').waitFor({state:'detached'});
  const rail=await page.getByLabel('Herramientas de captura').boundingBox();assert.equal(rail.width,42);assert.ok(rail.height<=438);
  assert.equal(await page.locator('.capture-toolbar').evaluate(e=>e.scrollHeight>e.clientHeight),false);
  await page.getByRole('button',{name:'Contraer herramientas'}).click();await page.getByRole('button',{name:'Expandir herramientas'}).click();
  await page.setViewportSize({width:350,height:380});assert.ok((await page.getByLabel('Herramientas de captura').boundingBox()).height<=372,'short screen toolbar has no internal scroll');
  assert.deepEqual(errors,[]);
  console.log('PASS: six real shapes exported to PNG, history, thin toolbar, nonduplicated actions, original Adobe custom palette and short-screen fit (mocked IPC).');
} finally {await browser.close();}
