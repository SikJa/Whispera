import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { captureHotkey, eventHotkey, formatHotkey, modifiers } from './hotkey-capture';
import './hotkey-input.css';

export default function HotkeyInput({id,value,onChange,disabled=false}:{id:string;value:string;onChange:(value:string)=>void;disabled?:boolean}) {
  const input=useRef<HTMLInputElement>(null), original=useRef(value), engaged=useRef(false), ready=useRef(false);
  const [listening,setListening]=useState(false),[held,setHeld]=useState(''),[error,setError]=useState('');
  const release=()=>{
    if(!engaged.current)return;
    engaged.current=false;ready.current=false;setListening(false);setHeld('');
    void captureHotkey(false).catch(e=>setError(String(e)));
  };
  useEffect(()=>{
    const blur=()=>input.current?.blur();window.addEventListener('blur',blur);window.addEventListener('pagehide',blur);
    return()=>{window.removeEventListener('blur',blur);window.removeEventListener('pagehide',blur);if(engaged.current){engaged.current=false;void captureHotkey(false).catch(()=>{});}};
  },[]);
  const keyDown=(e:KeyboardEvent<HTMLInputElement>)=>{
    if(e.key==='Tab'&&!e.ctrlKey&&!e.altKey&&!e.metaKey)return;
    e.preventDefault();e.stopPropagation();
    if(e.key==='Escape'){onChange(original.current);input.current?.blur();return;}
    if(!ready.current)return;
    const key=eventHotkey(e.nativeEvent);
    if(key){setHeld('');onChange(key);}
    else if(['Control','Alt','Shift','Meta'].includes(e.key))setHeld(formatHotkey(modifiers(e.nativeEvent).join('+'))+' + …');
  };
  return <div className="hotkey-field" data-listening={listening}>
    <input ref={input} id={id} type="text" readOnly disabled={disabled} autoComplete="off" spellCheck={false}
      value={held||formatHotkey(value)} placeholder="Presioná una combinación" aria-describedby={`${id}-hint`} aria-invalid={!!error}
      onFocus={()=>{
        original.current=value;engaged.current=true;ready.current=false;setError('');
        void captureHotkey(true).then(()=>{if(engaged.current){ready.current=true;setListening(true);}}).catch(e=>{if(engaged.current)setError(String(e));});
      }} onBlur={release} onKeyDown={keyDown} onKeyUp={()=>setHeld('')}/>
    <span id={`${id}-hint`} role="status">{error||(listening?'Presioná las teclas · Esc para cancelar':'Hacé clic y presioná tu atajo')}</span>
  </div>;
}
