export const backgrounds = [
  {id:'iris',name:'Iris',colors:['#a6dceb','#9863cc','#824cba','#6bd0e4']},
  {id:'rose',name:'Rosa',colors:['#f3bdb7','#de689a','#843999','#eda074']},
  {id:'ocean',name:'Oceano',colors:['#88e1dc','#258caf','#244785','#132749']},
  {id:'mint',name:'Menta',colors:['#ddf1bc','#9bd5bb','#438f92','#e5dec7']},
  {id:'sunset',name:'Atardecer',colors:['#ffd589','#f9897d','#d94f7e','#783c8b']},
  {id:'pearl',name:'Perla',colors:['#f1ebf5','#c4d6dc','#c4b8d5','#edf4ef']},
  {id:'cobalt',name:'Cobalto',colors:['#141f50','#324eaf','#588fce','#091726']},
  {id:'ember',name:'Carmesi',colors:['#3a1427','#a43d4b','#e68560','#35172b']},
  {id:'gold',name:'Ambar',colors:['#fce7a7','#eab76d','#b97254','#fbcea0']},
  {id:'lagoon',name:'Laguna',colors:['#cee9f0','#74bad9','#577ecc','#b9afd9']},
  {id:'orchid',name:'Orquidea',colors:['#f7c9e5','#bfb0f2','#6964b2','#efc2c7']},
  {id:'graphite',name:'Grafito',colors:['#666b76','#303641','#171d28','#858b91']},
];

// The same bitmap is used by preview and export, avoiding CSS/canvas gradient drift.
export function gradientBitmap(id:string,width:number,height:number):HTMLCanvasElement {
  const colors=(backgrounds.find(p=>p.id===id)||backgrounds[0]).colors;
  const small=document.createElement('canvas');small.width=96;small.height=64;
  const ctx=small.getContext('2d')!;const pixels=ctx.createImageData(96,64);
  const rgb=colors.map(hex=>[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16)));
  for(let y=0;y<64;y++)for(let x=0;x<96;x++){
    const u=x/95,v=y/63;
    const a=Math.max(0,Math.min(1,u+.18*Math.sin(v*Math.PI*2)));
    const b=Math.max(0,Math.min(1,v+.16*Math.sin(u*Math.PI*2)));
    const weights=[(1-a)*(1-b),a*(1-b),(1-a)*b,a*b];
    const offset=(y*96+x)*4;
    for(let c=0;c<3;c++)pixels.data[offset+c]=rgb.reduce((sum,color,i)=>sum+color[c]*weights[i],0);
    pixels.data[offset+3]=255;
  }
  ctx.putImageData(pixels,0,0);
  const output=document.createElement('canvas');output.width=width;output.height=height;
  const target=output.getContext('2d')!;target.imageSmoothingQuality='high';target.drawImage(small,0,0,width,height);
  return output;
}
