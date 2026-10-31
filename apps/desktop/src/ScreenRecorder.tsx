import ReplaySettings from './ReplaySettings';
import { FolderOpen } from 'lucide-react';
import {shallowEqual} from './shallow-equal';
import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { native } from './client';
import './screen-recorder.css';
import {useAutoSave,AutoSaveStatus} from './useAutoSave';
import { SettingsSwitch } from './ResourceControls';
import { type CapturePreferences as Preferences, defaultCapturePreferences, VideoAudioSettings } from './CaptureSettings';
import { commandKey, isMac, systemName } from './platform';

type Status = { phase: string; seconds: number; path: string; error: string; copied: boolean };
export {ScreenOverlay,ScreenSelection,ScreenIndicator} from './ScreenCaptureViews';

export default function ScreenRecorder() {
  const [windowsShortcuts, setWindowsShortcuts] = useState(false);
  const [preferencesReady,setPreferencesReady]=useState(!native);
  const [preferencesError,setPreferencesError]=useState('');
  const [preferencesAttempt,setPreferencesAttempt]=useState(0);
  const [preferences, setPreferences] = useState<Preferences>(defaultCapturePreferences);
  const autosave=useAutoSave<Preferences>(async p=>{
    if(!native)return;
    const saved=await invoke<Preferences>('screen_preferences');
    await invoke('screen_save_preferences',{preferences:{...saved,audio:p.audio,frame_color:p.frame_color,image_auto_copy:p.image_auto_copy}});
  });
  const changePreferences=(p:Preferences)=>{setPreferences(p);autosave.queue(p);};
  const [status, setStatus] = useState<Status>({ phase: 'idle', seconds: 0, path: '', error: '', copied: false });
  const [pending, setPending] = useState<{id:string;error:string}[]>([]);
  useEffect(()=>{
    if(!native)return;
    let alive=true;
    const refresh=()=>{if(document.visibilityState==='visible')void invoke<{id:string;error:string}[]>('screen_pending').then(value=>{if(alive&&Array.isArray(value))setPending(value);}).catch(()=>{});};
    refresh();const timer=setInterval(refresh,5000);
    return()=>{alive=false;clearInterval(timer);};
  },[]);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    setPreferencesReady(false);setPreferencesError('');
    Promise.all([invoke<Preferences>('screen_preferences'),isMac?Promise.resolve(false):invoke<boolean>('windows_capture_shortcuts')]).then(([p,keys]) => {
      if(alive){setPreferences({...defaultCapturePreferences,...p});setWindowsShortcuts(keys);setPreferencesReady(true);}
    }).catch(e => { if (alive) setPreferencesError(String(e)); });
    let timer:ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try { if(document.visibilityState==='visible'){const s=await invoke<Status>('screen_status');if(alive)setStatus(previous=>shallowEqual(previous,s)?previous:s);} }
      catch(e){if(alive)setMessage(String(e));}
      finally{if(alive)timer=setTimeout(refresh,500);}
    };
    void refresh();
    return () => { alive = false; clearInterval(timer); };
  }, [preferencesAttempt]);
  const run = async (action: () => Promise<unknown>, success = '') => {
    setBusy(true); setMessage('');
    try { await action(); setMessage(success); } catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  };
  const occupied = !preferencesReady || ['recording', 'paused', 'pausing', 'resuming', 'reframing', 'saving', 'cancelling', 'selecting', 'starting', 'editing'].includes(status.phase);
  return <section className="screen-recorder-panel">
    {preferencesError&&<p className="notice" role="alert">No se pudieron cargar las preferencias: {preferencesError} <button onClick={()=>setPreferencesAttempt(value=>value+1)}>Volver a cargar preferencias</button></p>}
    <>
      <h3>Audio predeterminado</h3>
      <VideoAudioSettings value={preferences.audio} disabled={occupied||busy} onChange={audio=>changePreferences({...preferences,audio})}/>
      <h3>Capturas de imagen</h3>
      {!isMac && <div className="form-row"><label htmlFor="windows-capture-shortcuts">Impr Pant para Whispera<span>Mientras Whispera está abierto, las capturas de Windows pasan a Ctrl + Alt + Shift + F12.</span></label><SettingsSwitch id="windows-capture-shortcuts" label="Impr Pant para Whispera" checked={windowsShortcuts} disabled={!native||occupied||busy} onChange={enabled=>void run(async()=>{await invoke('save_windows_capture_shortcuts',{enabled});setWindowsShortcuts(enabled);},'Atajos de captura guardados.')}/></div>}
      <div className="form-row"><label htmlFor="image-auto-copy">Copiar al soltar la selección<span>Va directo al portapapeles, sin abrir el editor.</span></label><SettingsSwitch id="image-auto-copy" label="Copiar al soltar la selección" checked={preferences.image_auto_copy} disabled={occupied||busy} onChange={image_auto_copy=>changePreferences({...preferences,image_auto_copy})}/></div>
      <div className="form-row"><label htmlFor="screen-frame-color">Color del recuadro<span>Para capturas y video. Blanco por defecto.</span></label><input id="screen-frame-color" type="color" value={preferences.frame_color} disabled={occupied||busy} onChange={e=>changePreferences({...preferences,frame_color:e.target.value})}/></div>
      <AutoSaveStatus save={autosave}/>
      <p className="muted-note">Seleccioná el área y grabá. Al detener, el video queda en el portapapeles para pegar con {commandKey}+V en aplicaciones que admitan archivos. Se conserva una copia temporal en esta computadora.</p>
    </>
    <div className="screen-record-actions">
      <button disabled={!native || busy || occupied} onClick={() => void run(() => invoke('screen_select'))}>Seleccionar área y grabar</button>
      <button disabled={!native||busy||(occupied&&!['recording','paused'].includes(status.phase))} onClick={()=>void run(()=>invoke('screen_select_image'))}>Capturar imagen</button>
      {status.phase === 'recording' && <p role="status">Grabando. Pulsá otra vez tu atajo para terminar.</p>}
      {status.phase === 'selecting' && <p role="status">Seleccioná el área. Escape cancela.</p>}
      {status.phase === 'saving' && <p role="status">Preparando video para pegar…</p>}
    </div>
    <p className="capture-shortcuts">Deshacer <kbd>{commandKey}+Z</kbd> · Rehacer <kbd>{commandKey}+Shift+Z</kbd> · Alinear trazos <kbd>Shift</kbd></p>

    {status.path && !occupied && <div className="screen-result"><p>{status.copied ? `Video copiado. Pegalo con ${commandKey}+V.` : 'Video disponible. Podés volver a copiarlo.'}</p><button onClick={() => void run(() => invoke('screen_copy'), 'Video copiado para pegar.')}>Copiar video</button><button onClick={() => void run(() => invoke('screen_reveal'))}>Ver archivo temporal</button></div>}
    {(message || status.error) && <p role="status" className="notice">{message || status.error}</p>}
    {pending.length>0&&<section aria-label="Videos pendientes de recuperación"><h3>Videos pendientes de recuperación</h3>{pending.map(item=><div className="form-row" key={item.id}><p>{item.error}</p><button disabled={busy} title="Ver originales conservados" onClick={()=>void run(()=>invoke('screen_pending_reveal',{id:item.id}))}><FolderOpen size={16}/>Ver originales</button></div>)}</section>}
    <ReplaySettings/>
    {!native && <p>La captura de pantalla requiere la aplicación de {systemName}.</p>}
  </section>;
}
