export type Capture = { key:string; path:string; src:string; width:number; height:number };
export type Layer = Capture & { x:number; y:number; w:number; h:number };
export type Composition = { width:number; height:number; background:string; padding:number; radius:number; shadow:number; blur?:number; backgroundSource?:string };
export function arrange(images:Capture[], options:Composition):Layer[] {
  if (!images.length) return [];
  const padding = Math.min(options.padding, options.height / 4);
  const gap = images.length > 1 ? 18 : 0;
  const cell = (options.width - padding * 2 - gap * (images.length - 1)) / images.length;
  return images.map((image,index) => {
    const ratio = image.width / image.height;
    const w = Math.max(1, Math.min(cell, (options.height - padding * 2) * ratio));
    const h = w / ratio;
    return {...image, x:padding + index * (cell + gap) + (cell - w) / 2,y:(options.height - h) / 2,w,h};
  });
}
export async function renderBackground(options:Composition,source?:string):Promise<HTMLCanvasElement> {
  const canvas=document.createElement('canvas');canvas.width=options.width;canvas.height=options.height;
  const ctx=canvas.getContext('2d')!;
  if(options.background.startsWith('gradient:'))ctx.drawImage(gradientBitmap(options.background.slice(9),canvas.width,canvas.height),0,0);
  else if((options.background==='blur'&&source)||(options.background==='wallpaper'&&options.backgroundSource)){
    const image=await loadImage(options.background==='wallpaper'?options.backgroundSource!:source!);const blur=options.background==='blur'?(options.blur??32):0;
    const scale=Math.max((canvas.width+blur*6)/image.width,(canvas.height+blur*6)/image.height);
    const w=image.width*scale,h=image.height*scale;
    ctx.filter=`blur(${blur}px)`;ctx.drawImage(image,(canvas.width-w)/2,(canvas.height-h)/2,w,h);ctx.filter='none';
  }else if(options.background!=='transparent'){
    ctx.fillStyle=['blur','wallpaper'].includes(options.background)?'#202024':options.background;ctx.fillRect(0,0,canvas.width,canvas.height);
  }
  return canvas;
}
export function loadImage(src:string):Promise<HTMLImageElement> {
  return new Promise((resolve,reject) => { const image = new Image(); image.crossOrigin = 'anonymous'; image.onload = () => resolve(image); image.onerror = () => reject(new Error('No se pudo abrir la imagen')); image.src = src; });
}
export async function renderComposition(layers:Layer[], options:Composition):Promise<HTMLCanvasElement> {
  const images = await Promise.all(layers.map(layer => loadImage(layer.src)));
  const canvas = document.createElement('canvas'); canvas.width = options.width; canvas.height = options.height;
  const ctx = canvas.getContext('2d')!;
  ctx.drawImage(await renderBackground(options,layers[0]?.src),0,0);
  layers.forEach((layer,index) => {
    const radius = Math.min(options.radius,layer.w / 2,layer.h / 2);
    const tile = document.createElement('canvas');tile.width=Math.ceil(layer.w);tile.height=Math.ceil(layer.h);
    const bitmap=tile.getContext('2d')!;bitmap.beginPath();bitmap.roundRect(0,0,layer.w,layer.h,radius);bitmap.clip();bitmap.drawImage(images[index],0,0,layer.w,layer.h);
    ctx.save();ctx.shadowColor=`rgba(0,0,0,${options.shadow/100})`;ctx.shadowBlur=30;ctx.shadowOffsetY=12;ctx.drawImage(tile,layer.x,layer.y,layer.w,layer.h);ctx.restore();
  });
  return canvas;
}
import { gradientBitmap } from './backgrounds';
