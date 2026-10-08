import {useEffect,useState} from 'react';
import {Minus,Square,Copy,X,PanelLeftClose,PanelLeftOpen} from 'lucide-react';
import {invoke} from '@tauri-apps/api/core';
import {native} from './client';
import './window-chrome.css';

export default function WindowChrome({onError,sidebarExpanded,onToggleSidebar}:{onError:(message:string)=>void;sidebarExpanded:boolean;onToggleSidebar:()=>void}) {
  const [maximized,setMaximized]=useState(false);
  useEffect(()=>{
    if(!native)return;
    const refresh=()=>void invoke<boolean>('settings_window_action',{action:'state'}).then(setMaximized).catch(()=>{});
    refresh();window.addEventListener('resize',refresh);
    return()=>window.removeEventListener('resize',refresh);
  },[]);
  async function action(action:string){
    if(!native)return;
    try{setMaximized(await invoke<boolean>('settings_window_action',{action}));}
    catch{onError('No se pudo cambiar el estado de la ventana.');}
  }
  return <header className="window-chrome" data-tauri-drag-region onDoubleClick={e=>{
    if(!(e.target as HTMLElement).closest('button'))void action('maximize');
  }}>
    <div className="window-chrome-left" data-tauri-drag-region><button className="sidebar-toggle" aria-label={sidebarExpanded?'Contraer sidebar':'Expandir sidebar'} aria-expanded={sidebarExpanded} onClick={onToggleSidebar}>{sidebarExpanded?<PanelLeftClose size={16}/>:<PanelLeftOpen size={16}/>}</button><span className="window-chrome-caption" data-tauri-drag-region>Configuración</span></div>
    <div className="window-chrome-controls">
      <button aria-label="Minimizar ventana" title="Minimizar" onClick={()=>void action('minimize')}><Minus size={15}/></button>
      <button aria-label={maximized?'Restaurar ventana':'Maximizar ventana'} title={maximized?'Restaurar':'Maximizar'} onClick={()=>void action('maximize')}>{maximized?<Copy size={13}/>:<Square size={13}/>}</button>
      <button className="window-chrome-close" aria-label="Cerrar configuración" title="Cerrar configuración" onClick={()=>void action('close')}><X size={16}/></button>
    </div>
  </header>;
}
