import {useEffect, useRef, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {Download, Headphones, Pause, Play, Square} from 'lucide-react';
import {Button} from '../vendor/components/ui/button';
import {native} from './client';
import './reader.css';

type Voice = {id:string; name:string; language:string; code?:string};
type ReaderStatus = {supported:boolean; phase:string; voices:Voice[]; exporting?:boolean; has_reading?:boolean};
type Preferences = {voice:string;rate:number;favorites:Record<string,string>};
type Analysis = {kind:string;language:string|null;voice:Voice|null};
const empty:ReaderStatus = {supported:false, phase:'idle', voices:[]};
// Only preferences are persisted. The reader text never enters localStorage.
function normalize(saved:Partial<Preferences>):Preferences {
  const favorites:Record<string,string>={};
  if(saved.favorites&&typeof saved.favorites==='object')for(const [code,id] of Object.entries(saved.favorites).slice(0,100))if(/^[a-z]{3}$/.test(code)&&typeof id==='string'&&id.length>0&&id.length<=512&&!/[\u0000-\u001f]/.test(id))favorites[code]=id;
  return {voice:typeof saved.voice==='string'&&saved.voice?saved.voice:'auto',rate:typeof saved.rate==='number'&&Number.isInteger(saved.rate)&&saved.rate>=-10&&saved.rate<=10?saved.rate:0,favorites};
}
function preferences():Preferences {
  try {return normalize(JSON.parse(localStorage.getItem('whispera.reader.preferences')||'{}')||{});}
  catch {return normalize({});}
}
function languageName(code:string):string {
  try {return new Intl.DisplayNames(['es'],{type:'language'}).of(code)||code;}catch{return code;}
}
export default function ReaderPanel({text,onTextChange,compact=false}:{text:string;onTextChange?:(text:string)=>void;compact?:boolean}) {
  const initial=useRef(preferences());
  const [status,setStatus]=useState<ReaderStatus>(empty), [voice,setVoice]=useState(initial.current.voice), [rate,setRate]=useState(initial.current.rate);
  const [loading,setLoading]=useState(native), [busy,setBusy]=useState(false), [error,setError]=useState(''), [reload,setReload]=useState(0);
  const [favorites,setFavorites]=useState(initial.current.favorites), [ready,setReady]=useState(!native);
  const [preview,setPreview]=useState<{key:string;analysis:Analysis}|null>(null);
  const saveQueue=useRef<Promise<void>>(Promise.resolve());
  const gate=useRef(false);
  const exportGate=useRef(false);
  const [saving,setSaving]=useState(false), [saved,setSaved]=useState('');
  useEffect(()=>{
    if(!native)return;setError("");let alive=true, pending=false, failed=false;
    const refresh=async()=>{
      if(pending||failed)return;pending=true;
      try {const next=await invoke<ReaderStatus>('reader_status');if(!alive)return;setStatus(next);
      }catch(reason){failed=true;if(alive)setError(String(reason));}
      finally{pending=false;if(alive)setLoading(false);}
    };
    void refresh();const timer=setInterval(()=>{if(!document.hidden&&!gate.current)void refresh();},1500);
    return()=>{alive=false;clearInterval(timer);};
  },[reload]);
  useEffect(()=>{
    if(!native)return;let alive=true;setReady(false);
    void invoke<Preferences>('reader_preferences').then(saved=>{if(!alive)return;const next=normalize(saved);setVoice(next.voice);setRate(next.rate);setFavorites(next.favorites);setReady(true);}).catch(reason=>{if(alive)setError(String(reason));});
    return()=>{alive=false;};
  },[reload]);
  useEffect(()=>{
    if(!ready)return;
    try{localStorage.setItem('whispera.reader.preferences',JSON.stringify({voice,rate,favorites}));}catch{/* Preferences are optional. */}
    if(native){const next=saveQueue.current.catch(()=>{}).then(()=>invoke<void>('reader_configure',{voice,rate,favorites}));saveQueue.current=next;void next.catch(reason=>setError(String(reason)));}
  },[voice,rate,favorites,ready]);
  const previewKey=JSON.stringify([text,voice,favorites,status.voices.map(item=>[item.id,item.code])]);
  useEffect(()=>{
    if(!native||!ready||voice!=='auto'||!text.trim()){setPreview(null);return;}
    let alive=true;
    const timer=setTimeout(()=>{void saveQueue.current.then(()=>invoke<Analysis>('reader_analyze',{text})).then(analysis=>{if(alive)setPreview({key:previewKey,analysis});}).catch(reason=>{if(alive){setPreview({key:previewKey,analysis:{kind:'error',language:null,voice:null}});setError(String(reason));}});},350);
    return()=>{alive=false;clearTimeout(timer);};
  },[previewKey,ready,reload]);
  // Closing the transcript/reader view explicitly stops its playback.
  useEffect(()=>()=>{if(native)void invoke('reader_control',{action:'stop'}).catch(()=>{});},[]);
  async function act(action:'speak'|'pause'|'resume'|'stop') {
    if(gate.current)return;gate.current=true;setBusy(true);setError('');
    try {if(action==='speak')await saveQueue.current;const next=action==='speak'
      ?await invoke<ReaderStatus>('reader_speak',{text,voice,rate})
      :await invoke<ReaderStatus>('reader_control',{action});setStatus(next);
    } catch(reason){setError(String(reason));}
    finally{gate.current=false;setBusy(false);}
  }
  async function saveMp3() {
    if(exportGate.current)return;exportGate.current=true;setSaving(true);setSaved('');setError('');
    // Snapshot current playback, including shortcut selections; otherwise export the editor.
    const reading=(status.phase==='speaking'||status.phase==='paused'||!text.trim())?null:{text,voice,rate};
    try {await saveQueue.current;const path=await invoke<string|null>('reader_export_mp3',{reading});setSaved(path?`MP3 guardado en ${path}`:'Guardado cancelado.');}
    catch(reason){setError(String(reason));}
    finally{exportGate.current=false;setSaving(false);setStatus(current=>({...current,exporting:false}));}
  }
  const playing=status.phase==='speaking', paused=status.phase==='paused', unavailable=!native||!status.supported||!status.voices.length||!ready;
  const analysis=preview?.key===previewKey?preview.analysis:null;
  const manualMissing=voice!=='auto'&&!status.voices.some(item=>item.id===voice);
  const autoBlocked=voice==='auto'&&!!text.trim()&&analysis?.kind!=='matched';
  const exportCurrent=playing||paused||(!text.trim()&&status.has_reading);
  const groups=[...new Set(status.voices.map(item=>item.code).filter((code):code is string=>!!code))].sort((a,b)=>languageName(a).localeCompare(languageName(b),'es'));
  return <section className={`reader-panel${compact?' reader-compact':''}`} aria-label="Lectura en voz alta">
    {!compact&&<div className="provider-line"><div className="provider-logo"><Headphones size={22}/></div><div><h2>Lectura local</h2><p>Voces instaladas en Windows · sin clave de Groq</p></div></div>}
    {!compact&&<p className="reader-privacy">Acceso rápido: seleccioná un texto y pulsá el atajo de Lectura configurado en Atajos (inicial: Ctrl + Alt + R). Volvé a pulsarlo para detener. Desde el icono de Whispera junto al reloj podés pausar, continuar o detener la lectura. No modifica el portapapeles.</p>}
    {!compact&&<p className="reader-privacy">Escuchá un texto o una transcripción. Podés guardar la lectura completa como MP3, también desde el icono junto al reloj. El texto no se envía a internet ni se guarda en el historial; la última lectura se conserva en memoria hasta cerrar Whispera.</p>}
    {onTextChange&&<label className="reader-text"><span>Texto para escuchar</span><textarea aria-label="Texto para escuchar" value={text} maxLength={50000} spellCheck={false} placeholder="Pegá o escribí el texto que querés escuchar…" onChange={event=>onTextChange(event.target.value)}/><small>{text.length.toLocaleString('es')} / 50.000 caracteres</small></label>}
    <div className="reader-options"><label><span>Voz</span><select aria-label="Voz de lectura" value={voice} disabled={loading||busy||!ready||!status.voices.length} onChange={event=>{const next=event.target.value;if(next==='auto'){const previous=status.voices.find(item=>item.id===voice);if(previous?.code&&!favorites[previous.code])setFavorites(current=>({...current,[previous.code!]:previous.id}));}setVoice(next);}}><option value="auto">Automático · detectar idioma</option>{manualMissing&&<option value={voice} disabled>Voz anterior no disponible</option>}{status.voices.map(item=><option key={item.id} value={item.id}>{item.name} · {item.language}</option>)}</select></label>
      <label><span>Velocidad <output>{rate===0?'Normal':rate>0?`+${rate}`:rate}</output></span><input type="range" aria-label="Velocidad de lectura" min={-10} max={10} step={1} value={rate} disabled={busy} onChange={event=>setRate(Number(event.target.value))}/></label></div>
    {voice==='auto'&&<><p className="reader-language-note" data-blocked={autoBlocked&&!!analysis} role="status" aria-live="polite">{!text.trim()?'Se detectará un idioma predominante por lectura, sin enviar el texto a internet.':!analysis?'Detectando idioma…':analysis.kind==='matched'?`Idioma detectado: ${languageName(analysis.language!)} · ${analysis.voice!.name}.`:analysis.kind==='missing_voice'?`Idioma detectado: ${languageName(analysis.language!)}. No hay una voz compatible instalada. Instalá una para ese idioma o elegí una voz manual.`:analysis.kind==='no_voices'?'No hay voces instaladas.':analysis.kind==='error'?'No se pudo comprobar el idioma. Volvé a intentarlo o elegí una voz manual.':'No se pudo identificar el idioma con suficiente seguridad. Probá un texto más largo o elegí una voz manual.'}</p>
      {groups.length>0&&<details className="reader-language-preferences"><summary>Voces preferidas por idioma</summary><p>La voz preferida define el acento. Si no está disponible, se usa otra voz instalada del mismo idioma.</p><div className="reader-options">{groups.map(code=>{const voices=status.voices.filter(item=>item.code===code);const favorite=favorites[code]||'';return <label key={code}><span>{languageName(code)}</span><select aria-label={`Voz preferida para ${languageName(code)}`} disabled={!ready||busy} value={favorite} onChange={event=>{const id=event.target.value;setFavorites(current=>{const next={...current};if(id)next[code]=id;else delete next[code];return next;});}}><option value="">Primera voz disponible</option>{favorite&&!voices.some(item=>item.id===favorite)&&<option value={favorite} disabled>Preferida no disponible · se usará otra compatible</option>}{voices.map(item=><option key={item.id} value={item.id}>{item.name} · {item.language}</option>)}</select></label>;})}</div></details>}</>}
    {manualMissing&&<p className="reader-language-note" role="status">La voz anterior ya no está disponible. Elegí otra voz o el modo Automático.</p>}
    <div className="reader-transport"><Button size="lg" disabled={unavailable||busy||manualMissing||autoBlocked||!text.trim()||playing||paused} onClick={()=>void act('speak')}><Play size={16}/>Leer en voz alta</Button>
      <Button variant="secondary" size="lg" disabled={unavailable||busy||(!playing&&!paused)} onClick={()=>void act(paused?'resume':'pause')}>{paused?<Play size={16}/>:<Pause size={16}/>} {paused?'Continuar':'Pausar'}</Button>
      <Button variant="secondary" size="lg" disabled={unavailable||busy||(!playing&&!paused)} onClick={()=>void act('stop')}><Square size={16}/>Detener</Button>
      <Button variant="secondary" size="lg" disabled={unavailable||busy||saving||status.exporting||(!exportCurrent&&(manualMissing||autoBlocked))||(!text.trim()&&!status.has_reading)} onClick={()=>void saveMp3()}><Download size={16}/>{saving||status.exporting?'Guardando MP3…':'Guardar como MP3'}</Button>
      {(saving||status.exporting)&&<Button variant="secondary" size="lg" onClick={()=>void invoke('reader_export_cancel').catch(reason=>setError(String(reason)))}>Cancelar guardado</Button>}</div>
    {(saving||status.exporting||saved)&&<p className="reader-export-state" role="status" aria-live="polite">{saving||status.exporting?'Generando la lectura completa. Podés seguir pausando o deteniendo la reproducción.':saved}</p>}
    <p className="reader-state" role="status" aria-live="polite">{loading?'Buscando el motor de Windows…':busy?'Actualizando lectura…':!native?'Vista previa: la lectura requiere la aplicación de Windows.':!ready?'Cargando preferencias…':!status.supported?'Esta versión ofrece lectura local en Windows.':!status.voices.length?'No hay voces compatibles. Instalá una voz desde la configuración de idioma de Windows.':playing?'Leyendo en voz alta.':paused?'Lectura pausada.':'Listo para leer. La voz y velocidad elegidas se aplican a la próxima lectura.'}</p>
    {error&&<div className="reader-error" role="alert"><p>{error}</p><button onClick={()=>setReload(value=>value+1)} disabled={busy}>Volver a comprobar</button></div>}
  </section>;
}
