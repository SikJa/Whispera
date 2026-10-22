import type {CSSProperties} from 'react';
export const appThemes = [
  {id:'plum',name:'Ciruela',background:'#211922',surface:'#2b2030',card:'#372939',ink:'#f8edf4',muted:'#c2afbf',accent:'#f4b59c'},
  {id:'graphite',name:'Grafito',background:'#1c1d20',surface:'#24262a',card:'#303238',ink:'#f0f1f3',muted:'#b5b9c2',accent:'#c6cedb'},
  {id:'navy',name:'Azul noche',background:'#17202d',surface:'#1d2a3a',card:'#29374a',ink:'#edf3fa',muted:'#acbdd0',accent:'#9fc5e8'},
  {id:'forest',name:'Bosque',background:'#19241f',surface:'#223129',card:'#2e4035',ink:'#eef5ee',muted:'#b0c3b4',accent:'#b5cdb2'},
  {id:'slate',name:'Pizarra',background:'#1b2427',surface:'#253135',card:'#324146',ink:'#edf4f5',muted:'#b0c3c7',accent:'#9fcece'},
  {id:'sand',name:'Arena',background:'#28221d',surface:'#332b24',card:'#44382e',ink:'#faf2e9',muted:'#c7b8a9',accent:'#ddc09c'},
] as const;
export function applyAppTheme(id?:string) {
  const theme=appThemes.find(t=>t.id===id)??appThemes[0];
  const root=document.documentElement;
  root.dataset.appTheme=theme.id;
  for(const key of ['background','surface','card','ink','muted','accent'] as const)root.style.setProperty(`--app-${key}`,theme[key]);
  try{localStorage.setItem('whispera.app.theme',theme.id);}catch{}
}
export function AppThemePicker({value,onChange}:{value?:string;onChange:(value:string)=>void}) {
  return <fieldset className="app-theme-picker"><legend>Tema de la aplicación</legend><p>Fondos, textos y acentos coordinados. Se guarda automáticamente.</p><div className="app-theme-options">
    {appThemes.map(theme=><button key={theme.id} type="button" aria-pressed={(value??'plum')===theme.id} onClick={()=>onChange(theme.id)} style={{'--theme-surface':theme.surface,'--theme-accent':theme.accent} as CSSProperties}><span className="app-theme-swatch" aria-hidden="true"><i/><i/><i/></span><span>{theme.name}</span></button>)}
  </div></fieldset>;
}
