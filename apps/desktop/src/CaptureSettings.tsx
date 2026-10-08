import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AudioLines, Camera, ClipboardList, Keyboard, Mic, Monitor, RefreshCw, VolumeX, Video } from 'lucide-react';
import { native } from './client';
import HotkeyInput from './HotkeyInput';
import { finishHotkeyCapture } from './hotkey-capture';
import {useAutoSave,AutoSaveStatus} from './useAutoSave';

export type CapturePreferences = { audio: 'none' | 'system' | 'microphone' | 'both'; hotkey: string; image_hotkey: string; frame_color: string; image_auto_copy: boolean };
export const defaultCapturePreferences: CapturePreferences = { audio:'none',hotkey:'Control+Shift+F9',image_hotkey:'Control+Shift+F10',frame_color:'#ffffff',image_auto_copy:false };
type Devices = { microphone:string|null; system:string|null };

export function ShortcutSettings({ voice, onSaved }: { voice:string; onSaved:()=>void }) {
  const [keys,setKeys] = useState({voice,video:defaultCapturePreferences.hotkey,image:defaultCapturePreferences.image_hotkey,library:'Alt+C'});
  const [loading,setLoading] = useState(native);
  const [busy,setBusy] = useState(false);
  const [message,setMessage] = useState('');
  useEffect(()=>{
    let alive=true;
    if(native) void Promise.all([invoke<CapturePreferences>('screen_preferences'),invoke<{toggleHotkey:string}>('library_preferences')]).then(([p,l])=>{if(alive)setKeys(k=>({...k,video:p.hotkey,image:p.image_hotkey,library:l.toggleHotkey}));}).catch(e=>{if(alive)setMessage(String(e));}).finally(()=>{if(alive)setLoading(false);});
    return()=>{alive=false;};
  },[]);
  const autosave=useAutoSave<typeof keys>(async next=>{
    setBusy(true);setMessage('');
    try { await finishHotkeyCapture(); await invoke('save_all_shortcuts', next); onSaved(); }
    finally{setBusy(false);}
  });
  return <section className="shortcut-settings">
    <p className="muted-note">Hacé clic en cada campo y presioná la combinación. Cada función usa un atajo distinto.</p>
    {([['voice','Dictado por voz','Iniciar y terminar la transcripción.',Keyboard],['video','Grabar video','Seleccionar un área y terminar la grabación.',Video],['image','Capturar imagen','Seleccionar un área de la pantalla.',Camera],['library','Portapapeles','Abrir y cerrar la biblioteca.',ClipboardList]] as const).map(([name,label,help,Icon])=><div className="form-row shortcut-setting-row" key={name}>
      <label htmlFor={`shortcut-${name}`}><Icon size={18}/>{label}<span>{help}</span></label>
      <HotkeyInput id={`shortcut-${name}`} disabled={busy||loading} value={keys[name]} onChange={value=>setKeys(k=>{const next={...k,[name]:value};autosave.queue(next);return next;})}/>
    </div>)}
    <AutoSaveStatus save={autosave}/>
    <p className="muted-note">Escape cierra una captura o termina el video. Al dibujar, V vuelve al puntero.</p>
    {message&&<p className="notice" role="status">{message}</p>}
  </section>;
}

export function VideoAudioSettings({value,disabled,onChange}:{value:CapturePreferences['audio'];disabled:boolean;onChange:(value:CapturePreferences['audio'])=>void}) {
  const [devices,setDevices]=useState<Devices>();
  const [error,setError]=useState('');
  const [loading,setLoading]=useState(false);
  const refresh=async()=>{setLoading(true);setError('');try{setDevices(await invoke<Devices>('screen_audio_devices'));}catch(e){setError(String(e));}finally{setLoading(false);}};
  useEffect(()=>{let alive=true;if(native)void invoke<Devices>('screen_audio_devices').then(d=>{if(alive)setDevices(d);}).catch(e=>{if(alive)setError(String(e));});return()=>{alive=false;};},[]);
  return <div className="video-audio-settings">
    <div className="audio-mode-grid" role="group" aria-label="Sonido del video">
      {([['none','Sin audio','Solo la imagen',VolumeX],['microphone','Micrófono','Tu voz',Mic],['system','Computadora','Sonido del sistema',Monitor],['both','Ambos','Tu voz y la computadora',AudioLines]] as const).map(([mode,label,hint,Icon])=><button key={mode} type="button" className="audio-mode" aria-pressed={value===mode} disabled={disabled} onClick={()=>onChange(mode)}><Icon size={19}/><strong>{label}</strong><small>{hint}</small></button>)}
    </div>
    <div className="capture-device-info" role="status"><div><span>Micrófono de Windows</span><strong>{devices?.microphone??(devices?'No se detectó un micrófono':'Detectando…')}</strong>{(value==='system'||value==='both')&&<><span>Salida de audio</span><strong>{devices?.system??'No se detectó una salida'}</strong></>}</div><button type="button" title="Actualizar dispositivos" aria-label="Actualizar dispositivos" disabled={!native||disabled||loading} onClick={()=>void refresh()}><RefreshCw size={16}/></button></div>
    {error&&<p role="alert" className="notice">{error}</p>}
  </div>;
}

type Recent = {id:string;kind:string;created_at:string;path:string};
export function CaptureHistory() {
  const [rows,setRows]=useState<Recent[]>([]);
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [message,setMessage]=useState('');
  useEffect(()=>{
    let alive=true;
    const refresh=()=>{if(!native){setLoading(false);return;}void invoke<Recent[]>('screen_recent').then(data=>{if(alive)setRows(data);}).catch(e=>{if(alive)setMessage(String(e));}).finally(()=>{if(alive)setLoading(false);});};
    refresh();window.addEventListener('focus',refresh);return()=>{alive=false;window.removeEventListener('focus',refresh);};
  },[]);
  const run=async(command:string,id:string)=>{setBusy(true);setMessage('');try{await invoke(command,{id});if(command==='screen_recent_copy')setMessage('Copiado. Pegalo con Ctrl+V.');}catch(e){setMessage(String(e));}finally{setBusy(false);}};
  return <section aria-label="Historial de capturas"><p className="muted-note">Las últimas 12 imágenes y videos, listos para volver a copiar.</p>
    {loading?<p role="status">Cargando capturas…</p>:rows.length?<div className="capture-recent-list">{rows.map(item=><div key={item.id} className="capture-recent-row"><span className="capture-file-kind">{item.kind==='image'?'PNG':'MP4'}</span><div><strong>{item.kind==='image'?'Captura de imagen':'Grabación de pantalla'}</strong><time>{new Date(item.created_at).toLocaleString('es-AR')}</time></div><button disabled={busy} onClick={()=>void run('screen_recent_copy',item.id)}>Copiar</button><button disabled={busy} title="Mostrar archivo" aria-label="Mostrar archivo" onClick={()=>void run('screen_recent_reveal',item.id)}>↗</button></div>)}</div>:<div className="empty-state"><Camera size={26}/><h2>Sin capturas todavía</h2><p>Las imágenes y videos que termines aparecen acá.</p></div>}
    {message&&<p className="notice" role="status">{message}</p>}
  </section>;
}
