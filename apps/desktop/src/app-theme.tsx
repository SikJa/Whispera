import type {CSSProperties} from 'react';
import PalettePanel from './PalettePanel';
import {selectionFromHex,selectionHex} from './palette';
import {appThemes,applyAppTheme} from './app-theme-data';
export {appThemes,customAppTheme,applyAppTheme} from './app-theme-data';
export function AppThemePicker({value,background,onChange,onBackgroundChange}:{value?:string;background?:string;onChange:(value:string)=>void;onBackgroundChange:(value:string)=>void}) {
  return <fieldset className="app-theme-picker"><legend>Tema de la aplicación</legend><p>Fondos, textos y acentos coordinados. Se guarda automáticamente.</p><div className="app-theme-options">
    {appThemes.map(theme=><button key={theme.id} type="button" aria-pressed={(value??'plum')===theme.id} onClick={()=>onChange(theme.id)} style={{'--theme-surface':theme.surface,'--theme-accent':theme.accent} as CSSProperties}><span className="app-theme-swatch" aria-hidden="true"><i/><i/><i/></span><span>{theme.name}</span></button>)}
  </div><div className="app-background-picker"><div><span>Fondo personalizado</span><p>Textos y superficies se adaptan para conservar el contraste.</p></div><div className="app-background-controls"><PalettePanel label="Elegir color de fondo" title="Color de fondo de la aplicación" value={selectionFromHex(background||(appThemes.find(t=>t.id===value)??appThemes[0]).background)} onChange={selection=>onBackgroundChange(selectionHex(selection))}/><button className="app-background-reset" type="button" disabled={!background} onClick={()=>onBackgroundChange('')}>Usar fondo del tema</button></div></div></fieldset>;
}
