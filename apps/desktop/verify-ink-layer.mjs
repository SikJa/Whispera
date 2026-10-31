import assert from 'node:assert/strict';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
const browser=await launchSilentBrowser();
try {
  const page=await browser.newPage();
  await page.goto((process.env.WHISPERA_TEST_URL||'http://127.0.0.1:5190')+'/overlay.html');
  const result=await page.evaluate(async()=>{
    const {InkLayer}=await import('/src/screen-ink-cache.ts');
    const {paint}=await import('/src/screen-annotations.ts');
    const marks=Array.from({length:30},(_,i)=>({id:String(i),tool:i%3?'pen':'highlight',width:3,color:i%2?'#123456':'#ff4545',points:Array.from({length:1000},(_,j)=>({x:j*.35,y:15+i*8+Math.sin(j*.02)*7}))}));
    marks.push({id:'text',tool:'text',color:'#ffffff',width:2,points:[{x:40,y:100}],text:'Synthetic text'});
    const layer=new InkLayer(),checks=[],times={direct:[],cached:[]};
    for(const scale of [1,1.25,2])for(const opaque of [true,false]) {
      const canvases=[document.createElement('canvas'),document.createElement('canvas')];
      for(const c of canvases){c.width=Math.round(400*scale);c.height=Math.round(320*scale);}
      const bg=ctx=>{if(opaque){ctx.fillStyle='#234567';ctx.fillRect(0,0,400,320);}};
      for(const excluded of [undefined,'2',undefined])for(const list of [marks,marks.slice(0,20),marks]) {
        canvases.forEach((c,index)=>{
          const ctx=c.getContext('2d');ctx.setTransform(scale,0,0,scale,0,0);ctx.clearRect(0,0,400,320);
          const cached=index?layer.render(list,excluded,400,320,scale,`${scale}:${opaque}`,null,bg):undefined;
          if(cached){ctx.setTransform(1,0,0,1,0,0);ctx.drawImage(cached,0,0);}
          else{bg(ctx);for(const m of list)if(m.id!==excluded)paint(ctx,m);}
        });
        const pixels=canvases.map(c=>c.getContext('2d').getImageData(0,0,c.width,c.height).data);
        checks.push(pixels[0].every((v,i)=>v===pixels[1][i]));
      }
    }
    const canvas=document.createElement('canvas');canvas.width=640;canvas.height=480;
    const ctx=canvas.getContext('2d');
    for(let trial=0;trial<11;trial++)for(const mode of trial%2?['cached','direct']:['direct','cached']) {
      const start=performance.now();
      for(let frame=0;frame<20;frame++){
        ctx.clearRect(0,0,640,480);
        if(mode==='cached')ctx.drawImage(layer.render(marks,undefined,640,480,1,'benchmark',null,()=>{}),0,0);
        else for(const mark of marks)paint(ctx,mark);
      }
      if(trial)times[mode].push((performance.now()-start)/20);
    }
    const released=layer.render(marks,undefined,640,480,1,'benchmark',null,()=>{});
    const oversized=layer.render(marks,undefined,9000,9000,1,'large',null,()=>{});
    const freed=released.width===0&&released.height===0;
    return {comparisons:checks.length,pixelIdentical:checks.every(Boolean),oversizedSkipped:oversized===undefined,freed,times};
  });
  assert.ok(result.pixelIdentical,'cached layers preserve exact RGBA at fractional scales, transparent/highlight and history/exclusion changes');
  assert.ok(result.oversizedSkipped&&result.freed,'oversized layers fall back and release cached bitmap');
  const median=v=>v.toSorted((a,b)=>a-b)[Math.floor(v.length/2)];
  console.log(JSON.stringify({...result,times:undefined,medianSubmissionMs:{direct:median(result.times.direct),cached:median(result.times.cached)},scope:'Canvas command submission only; excludes compositor/display, 30x1000 points, 20 frames/trial, 10 alternating measured trials'}));
} finally {await browser.close();}
