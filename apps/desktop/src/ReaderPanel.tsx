import {useEffect, useRef, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {Download, Headphones, Pause, Play, Square} from 'lucide-react';
import {Button} from '../vendor/components/ui/button';
import {native} from './client';
import './reader.css';

type Voice = {id:string; name:string; language:string};
type ReaderStatus = {supported:boolean; phase:string; voices:Voice[]; exporting?:boolean; has_reading?:boolean};
const empty:ReaderStatus = {supported:false, phase:'idle', voices:[]};
// Only preferences are persisted. The reader text never enters localStorage.
function preferences():{voice:string;rate:number} {
  try { const saved=JSON.parse(localStorage.getItem('whispera.reader.preferences')||'{}');
    return {voice:typeof saved.voice==='string'?saved.voice:'', rate:Number.isInteger(saved.rate)&&saved.rate>=-10&&saved.rate<=10?saved.rate:0};
  } catch { return {voice:'',rate:0}; }
}
export default function ReaderPanel({text,onTextChange,compact=false}:{text:string;onTextChange?:(text:string)=>void;compact?:boolean}) {
  const initial=useRef(preferences());
  const [status,setStatus]=useState<ReaderStatus>(empty), [voice,setVoice]=useState(initial.current.voice), [rate,setRate]=useState(initial.current.rate);
  const [loading,setLoading]=useState(native), [busy,setBusy]=useState(false), [error,setError]=useState(''), [reload,setReload]=useState(0);
  const gate=useRef(false);
  const exportGate=useRef(false);
  const [saving,setSaving]=useState(false), [saved,setSaved]=useState('');
  useEffect(()=>{
    if(!native)return;setError("");let alive=true, pending=false, failed=false;
    const refresh=async()=>{
      if(pending||failed)return;pending=true;
      try {const next=await invoke<ReaderStatus>('reader_status');if(!alive)return;setStatus(next);
        setVoice(current=>next.voices.some(v=>v.id===current)?current:(next.voices.find(v=>v.language.startsWith('es'))||next.voices[0])?.id||'');
      }catch(reason){failed=true;if(alive)setError(String(reason));}
      finally{pending=false;if(alive)setLoading(false);}
    };
    void refresh();const timer=setInterval(()=>{if(!document.hidden&&!gate.current)void refresh();},1500);
    return()=>{alive=false;clearInterval(timer);};
  },[reload]);
  useEffect(()=>{try{localStorage.setItem('whispera.reader.preferences',JSON.stringify({voice,rate}));}catch{/* Preferences are optional. */}if(native&&voice)void invoke('reader_configure',{voice,rate}).catch(reason=>setError(String(reason)));},[voice,rate]);
  // Closing the transcript/reader view explicitly stops its playback.
  useEffect(()=>()=>{if(native)void invoke('reader_control',{action:'stop'}).catch(()=>{});},[]);
  async function act(action:'speak'|'pause'|'resume'|'stop') {
    if(gate.current)return;gate.current=true;setBusy(true);setError('');
    try { const next=action==='speak'
      ?await invoke<ReaderStatus>('reader_speak',{text,voice,rate})
      :await invoke<ReaderStatus>('reader_control',{action});setStatus(next);
    } catch(reason){setError(String(reason));}
    finally{gate.current=false;setBusy(false);}
  }
  async function saveMp3() {
    if(exportGate.current)return;exportGate.current=true;setSaving(true);setSaved('');setError('');
    // Snapshot current playback, including shortcut selections; otherwise export the editor.
    const reading=(status.phase==='speaking'||status.phase==='paused'||!text.trim())?null:{text,voice,rate};
    try {const path=await invoke<string|null>('reader_export_mp3',{reading});setSaved(path?`MP3 guardado en ${path}`:'Guardado cancelado.');}
    catch(reason){setError(String(reason));}
    finally{exportGate.current=false;setSaving(false);setStatus(current=>({...current,exporting:false}));}
  }
  const playing=status.phase==='speaking', paused=status.phase==='paused', unavailable=!native||!status.supported||!voice;
  return <section className={`reader-panel${compact?' reader-compact':''}`} aria-label="Lectura en voz alta">
    {!compact&&<div className="provider-line"><div className="provider-logo"><Headphones size={22}/></div><div><h2>Lectura local</h2><p>Voces instaladas en Windows · sin clave de Groq</p></div></div>}
    {!compact&&<p className="reader-privacy">Acceso rápido: seleccioná un texto y pulsá el atajo de Lectura configurado en Atajos (inicial: Ctrl + Alt + R). Volvé a pulsarlo para detener. Desde el icono de Whispera junto al reloj podés pausar, continuar o detener la lectura. No modifica el portapapeles.</p>}
    {!compact&&<p className="reader-privacy">Escuchá un texto o una transcripción. Podés guardar la lectura completa como MP3, también desde el icono junto al reloj. El texto no se envía a internet ni se guarda en el historial; la última lectura se conserva en memoria hasta cerrar Whispera.</p>}
    {onTextChange&&<label className="reader-text"><span>Texto para escuchar</span><textarea aria-label="Texto para escuchar" value={text} maxLength={50000} spellCheck={false} placeholder="Pegá o escribí el texto que querés escuchar…" onChange={event=>onTextChange(event.target.value)}/><small>{text.length.toLocaleString('es')} / 50.000 caracteres</small></label>}
    {status.supported&&status.voices.length>0&&!status.voices.some(item=>item.language.startsWith("es"))&&<p className="reader-privacy">No hay una voz en español instalada. Podés añadirla en la configuración de idioma de Windows; el lector solo usa las voces disponibles.</p>}
    <div className="reader-options"><label><span>Voz</span><select aria-label="Voz de lectura" value={voice} disabled={loading||busy||!status.voices.length} onChange={event=>setVoice(event.target.value)}>{!status.voices.length&&<option value="">{loading?'Buscando voces…':'Sin voces disponibles'}</option>}{status.voices.map(item=><option key={item.id} value={item.id}>{item.name} · {item.language}</option>)}</select></label>
      <label><span>Velocidad <output>{rate===0?'Normal':rate>0?`+${rate}`:rate}</output></span><input type="range" aria-label="Velocidad de lectura" min={-10} max={10} step={1} value={rate} disabled={busy} onChange={event=>setRate(Number(event.target.value))}/></label></div>
    <div className="reader-transport"><Button size="lg" disabled={unavailable||busy||!text.trim()||playing||paused} onClick={()=>void act('speak')}><Play size={16}/>Leer en voz alta</Button>
      <Button variant="secondary" size="lg" disabled={unavailable||busy||(!playing&&!paused)} onClick={()=>void act(paused?'resume':'pause')}>{paused?<Play size={16}/>:<Pause size={16}/>} {paused?'Continuar':'Pausar'}</Button>
      <Button variant="secondary" size="lg" disabled={unavailable||busy||(!playing&&!paused)} onClick={()=>void act('stop')}><Square size={16}/>Detener</Button>
      <Button variant="secondary" size="lg" disabled={unavailable||busy||saving||status.exporting||(!text.trim()&&!status.has_reading)} onClick={()=>void saveMp3()}><Download size={16}/>{saving||status.exporting?'Guardando MP3…':'Guardar como MP3'}</Button>
      {(saving||status.exporting)&&<Button variant="secondary" size="lg" onClick={()=>void invoke('reader_export_cancel').catch(reason=>setError(String(reason)))}>Cancelar guardado</Button>}</div>
    {(saving||status.exporting||saved)&&<p className="reader-export-state" role="status" aria-live="polite">{saving||status.exporting?'Generando la lectura completa. Podés seguir pausando o deteniendo la reproducción.':saved}</p>}
    <p className="reader-state" role="status" aria-live="polite">{loading?'Buscando el motor de Windows…':busy?'Actualizando lectura…':!native?'Vista previa: la lectura requiere la aplicación de Windows.':!status.supported?'Esta versión ofrece lectura local en Windows.':!voice?'No hay voces compatibles. Instalá una voz desde la configuración de idioma de Windows.':playing?'Leyendo en voz alta.':paused?'Lectura pausada.':'Listo para leer. La voz y velocidad elegidas se aplican a la próxima lectura.'}</p>
    {error&&<div className="reader-error" role="alert"><p>{error}</p><button onClick={()=>setReload(value=>value+1)} disabled={busy}>Volver a comprobar</button></div>}
  </section>;
}
