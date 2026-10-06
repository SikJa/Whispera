import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ClipboardList } from 'lucide-react';
import { native } from './client';
import { SettingsSwitch } from './ResourceControls';
import {useAutoSave,AutoSaveStatus} from './useAutoSave';
import LibraryBrowser from './LibraryBrowser';
import './LibraryBrowser.css';

type Preferences = { captureGlobal:boolean; incognito:boolean; historyLimit:number; autoDeleteHours:number; clearUnpinnedOnRestart:boolean; transcribeVideo:boolean; videoTranscriptAttachment:boolean };
const defaults:Preferences = {captureGlobal:true,incognito:false,historyLimit:250,autoDeleteHours:48,clearUnpinnedOnRestart:false,transcribeVideo:true,videoTranscriptAttachment:true};
export default function LibrarySettings() {
  const [tab,setTab]=useState<'browser'|'settings'>('browser');
  const [value,setValue]=useState(defaults);
  const [loading,setLoading]=useState(native);
  const [ready,setReady]=useState(!native);
  const [loadError,setLoadError]=useState('');
  const [loadAttempt,setLoadAttempt]=useState(0);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  useEffect(()=>{let alive=true;if(native){setLoading(true);setReady(false);setLoadError('');void invoke<Partial<Preferences>>('library_preferences').then(p=>{if(alive){setValue({...defaults,...p});setReady(true);}}).catch(e=>{if(alive)setLoadError(String(e));}).finally(()=>{if(alive)setLoading(false);});}return()=>{alive=false;};},[loadAttempt]);
  const autosave=useAutoSave<Preferences>(async next=>{setBusy(true);try{const patch=Object.fromEntries(Object.keys(defaults).map(key=>[key,next[key as keyof Preferences]]));await invoke('library_action',{action:'settings',id:null,value:patch});}finally{setBusy(false);}});
  const changeValue=(change:(p:Preferences)=>Preferences)=>setValue(p=>{const next=change(p);autosave.queue(next);return next;});
  const toggle=(key:keyof Preferences,label:string,description:string)=><div className="form-row" key={key}><label htmlFor={`library-${key}`}>{label}<span>{description}</span></label><SettingsSwitch id={`library-${key}`} label={label} checked={Boolean(value[key])} onChange={checked=>changeValue(p=>({...p,[key]:checked}))}/></div>;
  return <section aria-label="Portapapeles" className="library-section">
    <div className="library-tabs" role="tablist" aria-label="Secciones del portapapeles" onKeyDown={event=>{
      if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
      event.preventDefault();
      const next=event.key==='Home'?'browser':event.key==='End'?'settings':tab==='browser'?'settings':'browser';
      setTab(next);document.getElementById(`library-tab-${next}`)?.focus();
    }}>
      <button id="library-tab-browser" role="tab" aria-selected={tab==='browser'} aria-controls="library-panel-browser" tabIndex={tab==='browser'?0:-1} onClick={()=>setTab('browser')}>Biblioteca</button>
      <button id="library-tab-settings" role="tab" aria-selected={tab==='settings'} aria-controls="library-panel-settings" tabIndex={tab==='settings'?0:-1} onClick={()=>setTab('settings')}>Configuración</button>
    </div>
    <div id="library-panel-browser" role="tabpanel" aria-labelledby="library-tab-browser" hidden={tab!=='browser'}>
      {tab==='browser'&&<LibraryBrowser/>}
    </div>
    <div id="library-panel-settings" role="tabpanel" aria-labelledby="library-tab-settings" hidden={tab!=='settings'}>
    {loadError&&<p className="notice" role="alert">{loadError} <button disabled={loading} onClick={()=>setLoadAttempt(attempt=>attempt+1)}>Reintentar lectura</button></p>}
    <fieldset disabled={!native||loading||!ready||busy} style={{border:0,padding:0,margin:0,minWidth:0}}>
      {toggle('captureGlobal','Guardar lo copiado en otras aplicaciones','Textos, enlaces, imágenes y archivos. Se guarda localmente.')}
      {toggle('incognito','Pausar el historial','No guardar nuevos elementos, incluidas las capturas de Whispera.')}
      <div className="form-row"><label htmlFor="library-limit">Capacidad del historial<span>Los elementos fijados se conservan.</span></label><input id="library-limit" type="number" min={10} max={1000} step={10} value={value.historyLimit} onChange={e=>changeValue(p=>({...p,historyLimit:Number(e.target.value)}))}/></div>
      <div className="form-row"><label htmlFor="library-retention">Eliminar automáticamente</label><select id="library-retention" value={value.autoDeleteHours} onChange={e=>changeValue(p=>({...p,autoDeleteHours:Number(e.target.value)}))}>{[0,24,48,168,720].map(hours=><option value={hours} key={hours}>{hours===0?'Nunca':hours<168?`${hours} horas`:`${hours/24} días`}</option>)}{![0,24,48,168,720].includes(value.autoDeleteHours)&&<option value={value.autoDeleteHours}>{value.autoDeleteHours} horas</option>}</select></div>
      {toggle('clearUnpinnedOnRestart','Limpiar al reiniciar','Conservar solamente los elementos fijados.')}
      <h3>Voz de los videos</h3>
      {toggle('transcribeVideo','Transcribir el micrófono con Groq','Solo videos nuevos con Micrófono o Ambos. Usa tu clave, modelo y diccionario.')}
      {toggle('videoTranscriptAttachment','Incluir transcripción al copiar el video','Adjunta un archivo .txt cuando esté listo. La aplicación de destino decide qué formatos acepta.')}
    </fieldset>
    <div className="page-actions"><AutoSaveStatus save={autosave}/><button disabled={!native} onClick={()=>void invoke('library_toggle').catch(e=>setError(String(e)))}><ClipboardList size={16}/>Abrir estante</button></div>
    {error&&<p className="notice" role="alert">{error}</p>}
    </div>
  </section>;
}
