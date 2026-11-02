import {useEffect,useId,useRef,useState} from 'react';
import {Search,ArrowUpRight,X} from 'lucide-react';
import {findSettings,sectionNames,type SettingResult} from './settings-search-index';
import './settings-search.css';

export default function SettingsSearch({onNavigate}:{onNavigate:(result:SettingResult)=>void}) {
  const [query,setQuery]=useState(''),[open,setOpen]=useState(false),[active,setActive]=useState(0);
  const root=useRef<HTMLDivElement>(null),input=useRef<HTMLInputElement>(null),listId=useId();
  const matches=findSettings(query),results=matches.slice(0,12),visible=open&&query.trim().length>0;
  useEffect(()=>{
    const outside=(event:PointerEvent)=>{if(!root.current?.contains(event.target as Node))setOpen(false);};
    const shortcut=(event:KeyboardEvent)=>{
      if(event.defaultPrevented||event.repeat||event.altKey||event.shiftKey||!(event.ctrlKey||event.metaKey)||event.key.toLowerCase()!=='k')return;
      if((event.target as HTMLElement)?.closest('.hotkey-field'))return;
      event.preventDefault();input.current?.focus();input.current?.select();setOpen(true);
    };
    document.addEventListener('pointerdown',outside);window.addEventListener('keydown',shortcut);
    return()=>{document.removeEventListener('pointerdown',outside);window.removeEventListener('keydown',shortcut);};
  },[]);
  useEffect(()=>{if(visible)document.getElementById(`${listId}-${active}`)?.scrollIntoView({block:'nearest'});},[active,visible,listId]);
  const choose=(result:SettingResult)=>{setQuery('');setOpen(false);input.current?.blur();onNavigate(result);};
  return <div className="settings-search" ref={root} onBlur={event=>{if(!event.currentTarget.contains(event.relatedTarget))setOpen(false);}}>
    <div className="settings-search-field"><Search size={17} aria-hidden="true"/>
      <input ref={input} type="text" role="combobox" aria-label="Buscar opciones" aria-autocomplete="list" aria-expanded={visible} aria-controls={visible?listId:undefined} aria-activedescendant={visible&&results[active]?`${listId}-${active}`:undefined}
        placeholder="Buscar opciones…" maxLength={120} autoComplete="off" spellCheck={false} value={query}
        onFocus={()=>setOpen(true)} onChange={event=>{setQuery(event.target.value);setActive(0);setOpen(true);}}
        onKeyDown={event=>{
          if(event.nativeEvent.isComposing)return;
          if(event.key==='Escape'){event.preventDefault();setOpen(false);setQuery('');}
          else if(event.key==='ArrowDown'||event.key==='ArrowUp'){event.preventDefault();setOpen(true);setActive(value=>results.length?(value+(event.key==='ArrowDown'?1:-1)+results.length)%results.length:0);}
          else if(event.key==='Enter'&&visible&&results[active]){event.preventDefault();choose(results[active]);}
        }}/>
      {query?<button type="button" aria-label="Limpiar búsqueda de opciones" onClick={()=>{setQuery('');setActive(0);input.current?.focus();}}><X size={15}/></button>:<span className="settings-search-hint" aria-hidden="true">Ctrl / ⌘ K</span>}
    </div>
    {visible&&<div className="settings-search-results"><p role="status">{matches.length?`${matches.length} resultados${matches.length>12?' · Mostrando los primeros 12. Afiná la búsqueda.':''}`:'Sin resultados. Probá con voz, audio, atajo o versión.'}</p>
      <ul id={listId} role="listbox" aria-label="Opciones encontradas">{results.map((result,index)=><li key={`${result.route}-${result.title}`} id={`${listId}-${index}`} role="option" aria-selected={active===index}
        onPointerMove={()=>setActive(index)} onMouseDown={event=>event.preventDefault()} onClick={()=>choose(result)}>
        <span><strong>{result.title}</strong><small>Configuración › {sectionNames[result.route]}</small></span><ArrowUpRight size={15} aria-hidden="true"/>
      </li>)}</ul>
    </div>}
  </div>;
}
