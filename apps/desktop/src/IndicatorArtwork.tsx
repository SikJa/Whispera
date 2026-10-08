import './indicator-artwork.css';
import {useId} from 'react';

export default function IndicatorArtwork({kind,active=false,compact=false,color='#ffffff',original=true}:{kind:'folder'|'camera';active?:boolean;compact?:boolean;color?:string;original?:boolean}) {
  const filterId='folder-tone-'+useId().replace(/:/g,'');
  const tinted=kind==='folder'&&!(original&&color.toLowerCase()==='#ffffff');
  const channels=[1,3,5].map(start=>parseInt(color.slice(start,start+2),16)/255);
  const table=(channel:number)=>[channel*.15,channel*.78,channel*.96,Math.min(1,channel+.18),1].join(' ');
  return <span className="indicator-artwork" data-kind={kind} data-active={active} data-compact={compact} data-hit="indicator-object">
    {kind==='folder'&&<svg width="0" height="0" aria-hidden="true" className="indicator-filter"><defs><filter id={filterId} colorInterpolationFilters="sRGB" x="0" y="0" width="100%" height="100%"><feColorMatrix type="matrix" values=".2126 .7152 .0722 0 0 .2126 .7152 .0722 0 0 .2126 .7152 .0722 0 0 0 0 0 1 0"/><feComponentTransfer><feFuncR type="table" tableValues={table(channels[0])}/><feFuncG type="table" tableValues={table(channels[1])}/><feFuncB type="table" tableValues={table(channels[2])}/></feComponentTransfer></filter></defs></svg>}
    <img src={`/indicators/${kind}.png`} alt={kind==='folder'?'Carpeta metálica':'Cámara de captura'} draggable={false} style={tinted?{filter:`url(#${filterId})`}:undefined}/>
  </span>;
}
