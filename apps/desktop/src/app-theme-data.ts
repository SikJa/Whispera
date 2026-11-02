// Theme tokens without UI, so light windows (the tray menu) can theme themselves.
import {validHex} from './palette';
export const appThemes = [
  {id:'plum',name:'Ciruela',background:'#211922',surface:'#2b2030',card:'#372939',ink:'#f8edf4',muted:'#c2afbf',accent:'#f4b59c'},
  {id:'graphite',name:'Grafito',background:'#1c1d20',surface:'#24262a',card:'#303238',ink:'#f0f1f3',muted:'#b5b9c2',accent:'#c6cedb'},
  {id:'navy',name:'Azul noche',background:'#17202d',surface:'#1d2a3a',card:'#29374a',ink:'#edf3fa',muted:'#acbdd0',accent:'#9fc5e8'},
  {id:'forest',name:'Bosque',background:'#19241f',surface:'#223129',card:'#2e4035',ink:'#eef5ee',muted:'#b0c3b4',accent:'#b5cdb2'},
  {id:'slate',name:'Pizarra',background:'#1b2427',surface:'#253135',card:'#324146',ink:'#edf4f5',muted:'#b0c3c7',accent:'#9fcece'},
  {id:'sand',name:'Arena',background:'#28221d',surface:'#332b24',card:'#44382e',ink:'#faf2e9',muted:'#c7b8a9',accent:'#ddc09c'},
] as const;
const rgb=(hex:string)=>[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16));
const mix=(a:string,b:string,amount:number)=>'#'+rgb(a).map((c,i)=>Math.round(c+(rgb(b)[i]-c)*amount).toString(16).padStart(2,'0')).join('');
const luminance=(hex:string)=>rgb(hex).map(c=>{c/=255;return c<=.04045?c/12.92:((c+.055)/1.055)**2.4;}).reduce((sum,c,i)=>sum+c*[.2126,.7152,.0722][i],0);
const contrast=(a:string,b:string)=>(Math.max(luminance(a),luminance(b))+.05)/(Math.min(luminance(a),luminance(b))+.05);
export function customAppTheme(background:string) {
  const ink=contrast('#ffffff',background)>contrast('#000000',background)?'#ffffff':'#000000';
  const near=ink==='#ffffff'?'#ffffff':'#000000',far=ink==='#ffffff'?'#000000':'#ffffff';
  const surface=(amount:number)=>{const color=mix(background,near,amount);return contrast(ink,color)>=4.5?color:mix(background,far,amount);};
  const colors={background,surface:surface(.04),card:surface(.08),ink,muted:ink,accent:ink};
  for(const name of ['muted','accent'] as const)for(const amount of [.28,.18,.08,0]) {
    const color=mix(ink,background,amount);
    if([colors.background,colors.surface,colors.card].every(bg=>contrast(color,bg)>=4.5)){colors[name]=color;break;}
  }
  return colors;
}
export function applyAppTheme(id?:string,background?:string) {
  const base=appThemes.find(t=>t.id===id)??appThemes[0];
  const theme=validHex(background)?{...base,...customAppTheme(background)}:base;
  const root=document.documentElement;
  root.dataset.appTheme=theme.id;
  for(const key of ['background','surface','card','ink','muted','accent'] as const)root.style.setProperty(`--app-${key}`,theme[key]);
  root.style.setProperty('--app-border',theme.ink+'26');
  try{localStorage.setItem('whispera.app.theme',theme.id);}catch{}
  try{localStorage.setItem('whispera.app.background',validHex(background)?background:'');}catch{}
}
