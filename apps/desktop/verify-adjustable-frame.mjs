import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';
import {PNG} from 'pngjs';
const browser=await launchSilentBrowser();
try {
  for(const kind of ['image','video']) {
    const page=await browser.newPage({viewport:{width:1100,height:800}});
    const errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(installNativeMock,{kind,adjustable:true});
    await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/?view=screen-select');
    await page.locator('.screen-selection').waitFor();
    await page.mouse.move(200,200);await page.mouse.down();await page.mouse.move(600,450);await page.mouse.up();
    const frame=page.getByTestId('recording-frame');await page.locator('.capture-frame-handle-se').waitFor();
    assert.equal(await page.locator('.capture-frame-handle').count(),8);
    assert.equal(await frame.evaluate(e=>getComputedStyle(e).borderRadius),'14px');
    assert.equal(await page.locator('.capture-border-beam').count(),1);
    async function assertBeamAligned() {
      const geometry=await frame.evaluate(e=>{
        const border=e.getBoundingClientRect(),svg=e.querySelector('svg'),path=svg.querySelector('rect'),box=svg.getBoundingClientRect();
        const thickness=parseFloat(getComputedStyle(e).borderTopWidth);
        return {left:box.x+path.x.baseVal.value-border.x,top:box.y+path.y.baseVal.value-border.y,right:box.x+path.x.baseVal.value+path.width.baseVal.value-border.x,bottom:box.y+path.y.baseVal.value+path.height.baseVal.value-border.y,thickness,width:border.width,height:border.height,radius:path.rx.baseVal.value,borderRadius:parseFloat(getComputedStyle(e).borderRadius)};
      });
      assert.equal(geometry.left,geometry.thickness/2,'shine follows the border centerline');
      assert.equal(geometry.top,geometry.thickness/2);
      assert.equal(geometry.right,geometry.width-geometry.thickness/2);
      assert.equal(geometry.bottom,geometry.height-geometry.thickness/2);
      assert.equal(geometry.radius,geometry.borderRadius-geometry.thickness/2);
    }
    await assertBeamAligned();
    async function drag(locator,dx,dy) {
      const box=await locator.boundingBox();const x=box.x+box.width/2,y=box.y+box.height/2;
      await frame.evaluate(e=>{window.originalBorder=e;window.originalBeam=e.querySelector('svg');});
      const committed=await page.evaluate(()=>structuredClone(window.editorContext.rect));
      const crops=await page.evaluate(()=>window.calls.filter(c=>c.command==='screen_resize_region').length);
      await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+dx,y+dy,{steps:4});
      await page.waitForFunction(()=>JSON.stringify(window.previewRect)!==JSON.stringify(window.editorContext.rect));
      assert.deepEqual(await page.evaluate(()=>window.editorContext.rect),committed,'drag previews do not recrop images or change the video region');
      assert.equal(await page.evaluate(()=>window.calls.filter(c=>c.command==='screen_resize_region').length),crops);
      assert.equal(await frame.evaluate(e=>e===window.originalBorder&&e.querySelector('svg')===window.originalBeam),true,'same animated border remains during dragging');
      assert.equal(await frame.evaluate(e=>getComputedStyle(e).borderColor),'rgb(255, 255, 255)');
      assert.equal(await frame.evaluate(e=>getComputedStyle(e).outlineStyle),'none');
      assert.equal(await page.locator('.capture-border-beam rect').first().evaluate(e=>getComputedStyle(e).animationName),'capture-border-travel');
      await assertBeamAligned();
      await page.mouse.up();
      await page.waitForFunction(()=>window.calls.at(-1)?.command==='screen_frame_drag'&&window.calls.at(-1).args.active===false);
    }
    await drag(page.locator('.capture-frame-handle-se'),100,50);
    assert.deepEqual(await frame.boundingBox(),{x:200,y:200,width:500,height:300});
    await drag(page.locator('.capture-frame-handle-nw'),-50,-40);
    assert.deepEqual(await frame.boundingBox(),{x:150,y:160,width:550,height:340});
    // Drag the top border away from its centered resize handle.
    await page.mouse.move(250,160);await page.mouse.down();await page.mouse.move(320,240,{steps:4});await page.mouse.up();
    await page.waitForFunction(()=>window.editorContext.rect.x===220);
    assert.deepEqual(await frame.boundingBox(),{x:220,y:240,width:550,height:340});
    assert.equal(await page.evaluate(()=>document.elementFromPoint(400,400)?.closest('button')!==null),false,'interior does not grab the move button');
    await page.evaluate(()=>window.failResize=true);
    await drag(page.locator('.capture-frame-handle-e'),30,0);
    await page.getByRole('alert').waitFor();
    assert.deepEqual(await frame.boundingBox(),{x:220,y:240,width:550,height:340},'failed native crop restores border');
    assert.deepEqual(errors,[]);await page.close();
  }
  const queued=await browser.newPage({viewport:{width:1100,height:800}});
  await queued.addInitScript(installNativeMock,{kind:'image',adjustable:true});
  await queued.addInitScript(()=>{
    const original=window.__TAURI_INTERNALS__.invoke;
    window.previewInFlight=0;window.previewMax=0;
    window.__TAURI_INTERNALS__.invoke=async(command,args)=>{
      if(command==='screen_frame_preview') {
        window.previewMax=Math.max(window.previewMax,++window.previewInFlight);
        await new Promise(resolve=>setTimeout(resolve,80));
        try{return await original(command,args);}finally{window.previewInFlight--;}
      }
      return original(command,args);
    };
  });
  await queued.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/?view=screen-select');
  await queued.locator('.screen-selection').waitFor();
  await queued.mouse.move(200,200);await queued.mouse.down();await queued.mouse.move(600,450);await queued.mouse.up();
  await queued.locator('.capture-frame-handle-se').waitFor();
  await queued.mouse.move(250,200);await queued.mouse.down();
  await queued.mouse.move(270,220);await queued.waitForFunction(()=>window.previewInFlight===1);
  await queued.mouse.move(290,240);await queued.mouse.move(320,270);await queued.mouse.up();
  await queued.waitForFunction(()=>window.calls.at(-1)?.command==='screen_frame_drag'&&window.calls.at(-1).args.active===false);
  assert.equal(await queued.evaluate(()=>window.previewMax),1,'slow native moves cannot accumulate concurrent preview calls');
  assert.deepEqual(await queued.evaluate(()=>window.previewRect),{x:270,y:270,width:400,height:250});
  const order=await queued.evaluate(()=>window.calls.filter(c=>['screen_frame_preview','screen_resize_region'].includes(c.command)).map(c=>c.command));
  assert.equal(order.at(-1),'screen_resize_region','release waits for the final dock preview before committing');
  await queued.close();
  const page=await browser.newPage();await page.goto(process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190');
  const cases=await page.evaluate(async()=>{
    const {adjustSelection}=await import('/src/selection-geometry.ts');
    const r={x:20,y:30,width:100,height:80},bounds={width:300,height:200};
    return {directions:['n','ne','e','se','s','sw','w','nw'].map(h=>adjustSelection(r,h,10,10,bounds)),move:adjustSelection(r,'move',999,-999,bounds),min:adjustSelection(r,'nw',999,999,bounds),max:adjustSelection(r,'se',999,999,bounds)};
  });
  assert.deepEqual(cases.directions,[{x:20,y:40,width:100,height:70},{x:20,y:40,width:110,height:70},{x:20,y:30,width:110,height:80},{x:20,y:30,width:110,height:90},{x:20,y:30,width:100,height:90},{x:30,y:30,width:90,height:90},{x:30,y:30,width:90,height:80},{x:30,y:40,width:90,height:70}]);
  assert.deepEqual(cases.move,{x:200,y:0,width:100,height:80});assert.deepEqual(cases.min,{x:104,y:94,width:16,height:16});assert.deepEqual(cases.max,{x:20,y:30,width:280,height:170});
  const ink=await browser.newPage({viewport:{width:400,height:300}});
  await ink.addInitScript(installNativeMock,{kind:'image',width:200,height:150,scale:2});
  await ink.addInitScript(()=>{window.editorContext.rect={x:50,y:60,width:200,height:150};});
  await ink.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html?view=screen-ink');
  await ink.waitForFunction(()=>window.calls.some(c=>c.command==='screen_editor_ready')&&!window.feedback.busy);
  await ink.keyboard.press('p');await ink.mouse.move(20,20);await ink.mouse.down();await ink.mouse.move(80,20,{steps:6});await ink.mouse.up();
  await ink.waitForFunction(()=>window.feedback.count===1);
  await ink.evaluate(()=>{window.editorContext={...window.editorContext,width:250,height:180,rect:{x:20,y:30,width:250,height:180}};window.emitNative('screen-editor-reset',window.editorContext);});
  await ink.waitForFunction(()=>document.querySelector('canvas').width===500&&!window.feedback.busy);
  await ink.keyboard.press('Control+z');await ink.waitForFunction(()=>window.feedback.count===0);
  await ink.keyboard.press('Control+y');await ink.waitForFunction(()=>window.feedback.count===1);
  await ink.keyboard.press('Control+c');await ink.waitForFunction(()=>window.exports.length===1);
  const png=PNG.sync.read(Buffer.from(await ink.evaluate(()=>window.exports[0].bytes)));
  assert.equal(png.width,500);assert.equal(png.height,360);
  const offset=(100*png.width+130)*4;
  assert.ok(png.data[offset]>240&&png.data[offset+1]<160,'annotation stays anchored to its original screen pixels after crop expansion and undo/redo');
  console.log('PASS: image/video resize, movement, eight handles, bounds, minimum size, unchanged rounded beam and error recovery. Native IPC mocked.');
}finally{await browser.close();}
