import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {open} from '@tauri-apps/plugin-dialog';
import {native} from './client';
import HotkeyInput from './HotkeyInput';
import {SettingsSwitch} from './ResourceControls';
import './replay.css';

type Preferences={enabled:boolean;seconds:number;audio:string;folder:string;hotkey:string};
type Status={phase:string;availableSeconds:number;encoder:string;error:string};
const defaults:Preferences={enabled:false,seconds:60,audio:'none',folder:'',hotkey:''};
export default function ReplaySettings(){
  const [value,setValue]=useState(defaults),[status,setStatus]=useState<Status>({phase:'off',availableSeconds:0,encoder:'',error:''});
  const [ready,setReady]=useState(false),[busy,setBusy]=useState(false),[message,setMessage]=useState('');
  const alive=useRef(true),gate=useRef(false);
  useEffect(()=>{alive.current=true;let off:(()=>void)|undefined;
    if(native){void Promise.all([invoke<Preferences>('replay_preferences'),invoke<Status>('replay_status')]).then(([prefs,state])=>{if(alive.current){setValue(prefs);setStatus(state);setReady(true);}}).catch(e=>{if(alive.current)setMessage(String(e));});
      void listen<Status>('replay-status',event=>{if(alive.current)setStatus(event.payload);}).then(fn=>{if(alive.current)off=fn;else fn();});
    }else setReady(true);
    return()=>{alive.current=false;off?.();};
  },[]);
  const save=async(next:Preferences)=>{if(gate.current)return;gate.current=true;setBusy(true);setMessage('');
    try{await invoke('replay_save_preferences',{value:next});if(alive.current){setValue(next);setMessage('Guardado automáticamente.');}}
    catch(e){if(alive.current)setMessage(String(e));}finally{gate.current=false;if(alive.current)setBusy(false);}
  };
  const folder=async()=>{if(gate.current)return;gate.current=true;setBusy(true);setMessage('');
    try{const parent=await open({directory:true,multiple:false,title:'Dónde crear la carpeta de repeticiones'});if(typeof parent==='string'){
      const path=await invoke<string>('replay_create_folder',{parent});const next={...value,folder:path};await invoke('replay_save_preferences',{value:next});if(alive.current){setValue(next);setMessage('Carpeta creada y guardada.');}
    }}catch(e){if(alive.current)setMessage(String(e));}finally{gate.current=false;if(alive.current)setBusy(false);}
  };
  const blocked=busy||!ready||!native;
  return <details className="replay-settings"><summary>Grabación hacia atrás <small>{value.enabled?'Activada':'Desactivada'}</small></summary>
    <p className="muted-note">Conservá los últimos segundos de tu pantalla principal. Al usar el atajo, elegís qué tramo guardar y copiar. Usa el codificador de video de tu GPU.</p>
    <div className="form-row"><label htmlFor="replay-enabled">Activar grabación hacia atrás<span>Requiere una carpeta y un atajo. Captura solamente mientras Whispera está abierto.</span></label><SettingsSwitch id="replay-enabled" label="Activar grabación hacia atrás" checked={value.enabled} disabled={blocked||!value.folder||!value.hotkey} onChange={enabled=>void save({...value,enabled})}/></div>
    <div className="form-row"><label htmlFor="replay-folder">Carpeta de repeticiones<span title={value.folder}>{value.folder||'Creá una carpeta para mantener tus videos organizados.'}</span></label><button id="replay-folder" disabled={blocked} onClick={()=>void folder()}>{value.folder?'Cambiar ubicación':'Crear carpeta'}</button></div>
    <div className="form-row"><label htmlFor="replay-hotkey">Guardar los últimos segundos</label><HotkeyInput id="replay-hotkey" value={value.hotkey} disabled={blocked} onChange={hotkey=>void save({...value,hotkey})}/></div>
    <div className="form-row"><label htmlFor="replay-seconds">Tiempo a conservar<span>Entre 5 segundos y 10 minutos.</span></label><input id="replay-seconds" type="number" min={5} max={600} defaultValue={value.seconds} key={value.seconds} disabled={blocked} onBlur={event=>{const seconds=Number(event.target.value);if(seconds!==value.seconds){if(seconds>=5&&seconds<=600&&Number.isInteger(seconds))void save({...value,seconds});else{event.target.value=String(value.seconds);setMessage('Elegí entre 5 y 600 segundos.');}}}}/></div>
    <div className="form-row"><label htmlFor="replay-audio">Audio de la repetición<span>Con audio, se transcribe únicamente el tramo que elegís.</span></label><select id="replay-audio" value={value.audio} disabled={blocked} onChange={e=>void save({...value,audio:e.target.value})}><option value="none">Sin audio</option><option value="system">Audio de la computadora</option><option value="microphone">Micrófono</option><option value="both">Computadora y micrófono</option></select></div>
    <div className="replay-status" role="status"><span>{status.phase==='buffering'?`${Math.floor(status.availableSeconds)} de ${value.seconds} segundos disponibles`:status.phase==='starting'?'Preparando grabación…':status.phase==='error'?'Grabación detenida':'Grabación desactivada'}</span>
      <button disabled={blocked||status.phase!=='buffering'||status.availableSeconds<1} onClick={()=>{setMessage('Preparando el video…');void invoke('replay_save').then(()=>{if(alive.current)setMessage('Preparación solicitada. El editor se abre automáticamente.');}).catch(e=>{if(alive.current)setMessage(String(e));});}}>Elegir tramo</button>
    </div>
    {(message||status.error)&&<p className="notice" role="status">{status.error||message}</p>}
  </details>;
}
