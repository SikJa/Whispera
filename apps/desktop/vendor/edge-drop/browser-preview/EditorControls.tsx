import React, { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown, RectangleHorizontal, RectangleVertical, Square } from 'lucide-react';
import './editor-controls.css';

export function EditorRange({label,value,min,max,unit,onChange}:{label:string;value:number;min:number;max:number;unit:string;onChange:(value:number)=>void}) {
  return <label className="editor-range"><span>{label}<output>{value}<small>{unit}</small></output></span><input type="range" aria-label={label} min={min} max={max} value={value} style={{'--range-fill':`${(value-min)/(max-min)*100}%`} as CSSProperties} onChange={e=>onChange(Number(e.target.value))}/></label>;
}

export function EditorToggle({checked,onChange}:{checked:boolean;onChange:(checked:boolean)=>void}) {
  const [touched,setTouched]=useState(false);
  return <div className="editor-balance"><span>Equilibrado automatico</span><button className={`t-toggle${touched?' is-init':''}`} role="switch" aria-label="Equilibrado automatico" aria-checked={checked} data-on={checked} onClick={()=>{setTouched(true);onChange(!checked);}}><span className="t-toggle-thumb"/></button></div>;
}

const ratios=[{value:1.6,label:'Original',icon:RectangleHorizontal},{value:16/9,label:'16:9',icon:RectangleHorizontal},{value:1,label:'1:1',icon:Square},{value:4/5,label:'4:5',icon:RectangleVertical},{value:9/16,label:'9:16',icon:RectangleVertical}];
export function RatioSelect({value,onChange}:{value:number;onChange:(value:number)=>void}) {
  const [state,setState]=useState<'closed'|'open'|'closing'>('closed');
  const [position,setPosition]=useState({left:0,top:0});
  const trigger=useRef<HTMLButtonElement>(null),menu=useRef<HTMLDivElement>(null);
  const timer=useRef<ReturnType<typeof setTimeout>>();
  const chosen=ratios.find(r=>Math.abs(r.value-value)<.002)||ratios[0];
  const close=(focus=false)=>{clearTimeout(timer.current);setState('closing');if(focus)trigger.current?.focus();const ms=matchMedia('(prefers-reduced-motion: reduce)').matches?0:parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--dropdown-close-dur'))||150;timer.current=setTimeout(()=>setState('closed'),ms);};
  useEffect(()=>()=>clearTimeout(timer.current),[]);
  useEffect(()=>{
    if(state!=='open')return;
    const outside=(e:PointerEvent)=>{if(!trigger.current?.contains(e.target as Node)&&!menu.current?.contains(e.target as Node))close();};
    const escape=(e:KeyboardEvent)=>{if(e.key==='Escape'){e.preventDefault();e.stopImmediatePropagation();close(true);}};
    document.addEventListener('pointerdown',outside);document.addEventListener('keydown',escape,true);
    menu.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus();
    return()=>{document.removeEventListener('pointerdown',outside);document.removeEventListener('keydown',escape,true);};
  },[state]);
  const open=()=>{clearTimeout(timer.current);const rect=trigger.current!.getBoundingClientRect();setPosition({left:Math.max(8,Math.min(innerWidth-184,rect.right-176)),top:rect.bottom+218>innerHeight?Math.max(8,rect.top-218):rect.bottom+6});setState('open');};
  return <div className="editor-ratio"><span>Proporcion</span><button ref={trigger} aria-label="Proporcion" aria-haspopup="menu" aria-expanded={state==='open'} onClick={()=>state==='open'?close():open()} onKeyDown={e=>{if(e.key==='ArrowDown'){e.preventDefault();open();}}}><chosen.icon size={15}/><span>{chosen.label}</span><ChevronDown size={13}/></button>
    {state!=='closed'&&createPortal(<div ref={menu} role="menu" aria-label="Proporcion del lienzo" className={`editor-ratio-menu t-dropdown ${state==='open'?'is-open':'is-closing'}`} data-origin="top-right" style={position} onKeyDown={e=>{
      if(e.key==='Tab'){close();return;}if(!['ArrowDown','ArrowUp','Home','End'].includes(e.key))return;e.preventDefault();
      const buttons=Array.from(menu.current!.querySelectorAll<HTMLButtonElement>('button'));const index=buttons.indexOf(document.activeElement as HTMLButtonElement);
      buttons[e.key==='Home'?0:e.key==='End'?buttons.length-1:(index+(e.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length]?.focus();
    }}>{ratios.map(r=><button key={r.label} role="menuitemradio" aria-checked={r.label===chosen.label} onClick={()=>{onChange(r.value);close(true);}}><r.icon size={16}/><span>{r.label}</span>{r.label===chosen.label&&<Check size={14}/>}</button>)}</div>,document.body)}
  </div>;
}
