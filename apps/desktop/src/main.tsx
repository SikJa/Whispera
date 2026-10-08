import VideoTrim from './VideoTrim';
import UpdatesPanel, {useUpdates} from "./UpdatesPanel";
import {listen as listenUpdate} from "@tauri-apps/api/event";
import React, { useEffect, useState, useRef } from "react";
import {useAutoSave,AutoSaveStatus} from './useAutoSave';
import { createRoot } from "react-dom/client";
import { Dialog, Modal, ModalOverlay } from "react-aria-components";
import { AudioLines, BookOpen, Palette, Keyboard, History, Activity, ArrowUpRight, Search, Plus, Trash2, Check, Upload, X, Paintbrush, Download, Settings2 } from "lucide-react";
import { CopyButton, SaveButton, SettingsSwitch } from "./ResourceControls";
import { Button } from "../vendor/components/ui/button";
import "@fontsource-variable/instrument-sans";
import ControlledFolder from "./ControlledFolder";
import IndicatorArtwork from "./IndicatorArtwork";
import PalettePanel from "./PalettePanel";
import RecorderPreview from "./RecorderPreview";
import Recorder from "./Recorder";
import FloatingRecorder from "./FloatingRecorder";
import FileImport from './FileImport';
import ScreenRecorder, { ScreenOverlay, ScreenIndicator } from './ScreenRecorder';
import { ScreenInk, ScreenTools, ScreenHud } from './ScreenEditor';
import ScreenFrozen from './ScreenFrozen';
import SoundLab from "./SoundLab";
import SoundSettings from "./SoundSettings";
import WindowChrome from "./WindowChrome";
import SettingsSidebar from "./SettingsSidebar";
import { Pencil, Volume2, RotateCcw, Play, ChevronLeft, ChevronRight, ClipboardList as CopyButtonIcon } from 'lucide-react';
import { invoke } from "@tauri-apps/api/core";
import { selectionHex } from "./palette";
import * as api from "./client";
import "./style.css";
import "./desktop.css";
import { SetupGate } from './Onboarding';
import GroqKeyGuide from './GroqKeyGuide';
import { ShortcutSettings, CaptureHistory } from './CaptureSettings';
import LibrarySettings from './LibrarySettings';
import ApplicationSettings from './ApplicationSettings';

const routes = [
  { id: "general", name: "General", icon: Activity, group: "Aplicación" },
  { id: "transcription", name: "Transcripción", icon: AudioLines, group: "Preferencias" },
  { id: "appearance", name: "Apariencia", icon: Palette },
  { id: "hotkey", name: "Atajos", icon: Keyboard },
  { id: "screen", name: "Capturas y video", icon: Play },
  { id: "sounds", name: "Sonidos", icon: Volume2 },
  { id: "dictionary", name: "Diccionario personal", icon: BookOpen, group: "Tu espacio" },
  { id: "history", name: "Historial", icon: History },
  { id: "library", name: "Portapapeles", icon: CopyButtonIcon },
  { id: "application", name: "Aplicación", icon: Settings2, group: "Aplicación" },
  { id: "updates", name: "Actualizaciones", icon: Download },
  { id: "diagnostics", name: "Diagnóstico", icon: Activity },
] as const;
type Route = typeof routes[number]["id"];
const artworkModels = [{id:'original',name:'Carpeta animada original'},{id:'metallic',name:'Carpeta metálica'}] as const;
const descriptions: Record<Route, string> = {
  general: "Inicio y comportamiento de Whispera.",
  transcription: "Tu voz, con tus preferencias.", dictionary: "Las palabras que tienen que salir bien.",
  appearance: "Tus indicadores, a tu manera.",
  hotkey: "Dictado, video y capturas, cada uno con su combinación.", history: "Transcripciones, capturas y videos, en un lugar.",
  diagnostics: "El estado de Whispera.", updates: "Siempre al día, sin salir de la aplicación.",
  sounds: "Inicio y fin del dictado.",
  screen: "Seleccioná, marcá y pegá. Imagen o video, con tus atajos.",
  library: "Tu historial local de textos, imágenes, videos y archivos.",
  application: "Cómo se inicia Whispera en tu computadora.",
};

function SettingsApp({sidebarExpanded}:{sidebarExpanded:boolean}) {
  const updater=useUpdates();
  const [historyKind,setHistoryKind] = useState<'text'|'captures'>('text');
  const [data, setData] = useState<api.Snapshot>();
  const [route, setRoute] = useState<Route>("transcription");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [key, setKey] = useState("");
  const [search, setSearch] = useState("");
  const [source, setSource] = useState("");
  const [target, setTarget] = useState("");
  const [editingRule, setEditingRule] = useState<string>();
  const [selected, setSelected] = useState<api.Transcript>();
  const [historyLimit,setHistoryLimit]=useState(50);
  const dirty=useRef<Partial<api.Settings>>({});
  const autosave=useAutoSave<Partial<api.Settings>>(async edit=>{
    await api.saveSettings({...await api.readSettings(),...edit});
    for(const name of Object.keys(edit) as (keyof api.Settings)[])if(dirty.current[name]===edit[name])delete dirty.current[name];
  });
  const refresh = () => api.snapshot().then(value=>setData({...value,settings:{...value.settings,...dirty.current}}));
  useEffect(() => { refresh().catch(e => setMessage(String(e))); }, []);
  useEffect(()=>{setHistoryLimit(50);},[route,search]);
  useEffect(() => {
    if (route !== 'dictionary') return;
    let alive = true;
    const reload = () => void api.readRules().then(rules => {
      if (alive) setData(data => data ? { ...data, rules } : data);
    }).catch(error => { if (alive) setMessage(String(error)); });
    reload();
    window.addEventListener('focus', reload);
    return () => { alive = false; window.removeEventListener('focus', reload); };
  }, [route]);
  useEffect(()=>{
    if(route!=='history'||!api.native)return;let alive=true;
    void invoke<api.Transcript[]>('read_history').then(history=>{if(alive)setData(d=>d?{...d,history}:d);}).catch(e=>{if(alive)setMessage(String(e));});
    return()=>{alive=false;};
  },[route]);
  useEffect(()=>{if(!api.native)return;let alive=true;let off:(()=>void)|undefined;void listenUpdate('updater-open',()=>{if(alive)setRoute('updates');},{target:'main'}).then(remove=>{if(alive)off=remove;else remove();});return()=>{alive=false;off?.();};},[]);
  const run = async (work: () => Promise<unknown>, success: string) => {
    setBusy(true);
    try { await work(); await refresh(); setMessage(success); return true; }
    catch (error) {
      if (route === 'dictionary') {
        try { const rules = await api.readRules(); setData(data => data ? { ...data, rules } : data); }
        catch { /* Preserve the last loaded rules if refreshing also fails. */ }
      }
      setMessage(String(error)); return false;
    } finally { setBusy(false); }
  };
  if (!data) return <div className="desktop-loading" role="status">{message || "Cargando configuración…"}</div>;
  const patch = (change: Partial<api.Settings>) => {dirty.current={...dirty.current,...change};autosave.queue({...dirty.current});setData(d=>d?{...d,settings:{...d.settings,...change}}:d);};
  const appearanceColor=data.settings.dictationArtwork==='metallic'?(data.settings.metallicColor??'#ffffff'):data.settings.color;
  const artworkIndex=Math.max(0,artworkModels.findIndex(model=>model.id===(data.settings.dictationArtwork??'original')));
  const changeArtwork=(direction:number)=>patch({dictationArtwork:artworkModels[(artworkIndex+direction+artworkModels.length)%artworkModels.length].id});
  const rules = data.rules.filter(r => `${r.source} ${r.target}`.toLowerCase().includes(search.toLowerCase()));
  const history = data.history.filter(r => r.text.toLowerCase().includes(search.toLowerCase()));
  const title = routes.find(r => r.id === route)!.name;

  return <div className="desktop-app" style={{ "--accent": "#f4b59c" } as React.CSSProperties}>
    <SettingsSidebar items={routes} value={route} expanded={sidebarExpanded} count={data.rules.length}
      onChange={id=>{setRoute(id as Route);setSearch('');setMessage('');}}
      brand={<a className="desktop-brand" href="?view=settings"><span className="brand-mark"><img src="/cristal/64x64.png" alt="" /></span><span className="brand-copy"><strong>Whispera</strong><small>Voz, capturas y video</small></span></a>}
      update={updater.status.version&&<button className="sidebar-update" aria-label="Actualización disponible" onClick={()=>setRoute('updates')}><Download size={16}/><span><strong>Actualización disponible</strong><small>Whispera {updater.status.version}</small></span><ArrowUpRight size={14}/></button>}
      footer={<div className="sidebar-bottom"><a className="recorder-link" href="?view=record" aria-label="Abrir o cerrar grabadora" aria-disabled={busy} onClick={e=>{if(api.native){e.preventDefault();if(!busy)void run(()=>invoke('toggle_recorder'),'');}}}><AudioLines size={18} /><span>Abrir grabadora</span><ArrowUpRight size={15} /></a></div>} />
    <div className="desktop-main" role="region" aria-label="Contenido de configuración" tabIndex={0}>
      <section className="desktop-content">
        <div className="page-heading"><div><h1>{title}</h1><p>{descriptions[route]}</p></div><AutoSaveStatus save={autosave}/></div>
        {message && <div className="notice" role="status">{message}<button aria-label="Cerrar aviso" onClick={() => setMessage("")}><X size={14} /></button></div>}
        <div className="settings-section">
        {route==='updates'&&<UpdatesPanel updater={updater}/>}
        {route==='application'&&<ApplicationSettings/>}
        {route === 'screen' && <ScreenRecorder />}
        {route === 'library' && <LibrarySettings />}
        {route==='sounds'&&<SoundSettings settings={data.settings} patch={patch} onError={setMessage}/>}
        {route==='diagnostics'&&<><div className="form-row"><label htmlFor="watchdog">Recuperar interfaz sin respuesta</label><SettingsSwitch id="watchdog" label="Vigilancia de interfaz" checked={data.settings.watchdog} onChange={v=>patch({watchdog:v})}/></div><button disabled={!api.native||busy} onClick={()=>run(()=>invoke('restart_app'),'Reiniciando')}><RotateCcw size={16}/>Reiniciar Whispera</button></>}
        {route === "transcription" && <>
          <div className="provider-line"><div className="provider-logo"><AudioLines size={22} /></div><div><h2>Groq</h2><p>Proveedor de transcripción</p></div><span className="state-tag"><i />{data.keyConfigured ? "Clave guardada" : "Sin conectar"}</span></div>
          <h3>Voz e idioma</h3>
          <div className="form-row"><label htmlFor="model">Modelo</label><select id="model" value={data.settings.model} onChange={e => patch({ model: e.target.value })}><option value="whisper-large-v3-turbo">Whisper Large v3 Turbo</option><option value="whisper-large-v3">Whisper Large v3</option></select></div>
          <div className="form-row"><label htmlFor="language">Idioma del audio</label><select id="language" value={data.settings.language} onChange={e => patch({ language: e.target.value })}><option value="es">Español</option><option value="en">English</option><option value="pt">Português</option><option value="auto">Detectar automáticamente</option></select></div>
          <h3>Conexión</h3><div className="key-line"><label htmlFor="key">Clave API de Groq <span>Almacenada en Windows</span></label><div><input id="key" type="password" autoComplete="off" value={key} placeholder={data.keyConfigured ? "••••••••••••••••" : "gsk_…"} onChange={e => setKey(e.target.value)} onBlur={()=>{if(key.trim()&&api.native)void run(async()=>{await api.saveKey(key);setKey("");},"Clave guardada automáticamente en Windows");}} /></div></div>
          
          <GroqKeyGuide />

          <h3>Al terminar</h3><div className="form-row"><label htmlFor="copy">Copiar automáticamente<span>El texto queda en tu portapapeles.</span></label><SettingsSwitch id="copy" label="Copiar al finalizar" checked={data.settings.autoCopy} onChange={checked => patch({ autoCopy: checked })} /></div>
          <div className="form-row"><label htmlFor="paste">Pegar en el destino original</label><SettingsSwitch id="paste" label="Pegar al finalizar dictado" checked={data.settings.autoPaste} onChange={checked=>patch({autoPaste:checked})}/></div>
          <div className="form-row"><label htmlFor="incremental">Transcribir mientras grabo</label><SettingsSwitch id="incremental" label="Transcripcion anticipada" checked={data.settings.incrementalTranscription} onChange={checked=>patch({incrementalTranscription:checked})}/></div>
          <div className="form-row"><label htmlFor="trim">Reducir silencios en el envío</label><SettingsSwitch id="trim" label="Recortar silencios" checked={data.settings.trimSilence} onChange={checked=>patch({trimSilence:checked})}/></div>
          <div className="page-actions"><Button variant="secondary" size="lg" disabled={busy || !api.native} onClick={() => run(()=>invoke('open_import'), "Ventana de audio abierta") }><Upload data-icon="inline-start" />Transcribir archivo</Button></div>
        </>}

        {route === "dictionary" && <>
          <div className="list-toolbar"><label className="search-field"><Search size={16} /><input aria-label="Buscar corrección" placeholder="Buscar palabra o sustitución" value={search} onChange={e => setSearch(e.target.value)} /></label><span>{data.rules.length} reglas</span></div>
          <form className="dictionary-add" onSubmit={e => { e.preventDefault(); if (!busy && source.trim() && target.trim()) run(async () => { const updated={id:editingRule??crypto.randomUUID(),source:source.trim(),target:target.trim(),enabled:data.rules.find(r=>r.id===editingRule)?.enabled??true}; await api.saveRules(editingRule?data.rules.map(r=>r.id===editingRule?updated:r):[...data.rules,updated], data.rules); setSource(""); setTarget(""); setEditingRule(undefined); }, "Corrección guardada"); }}><input aria-label="Palabra detectada" placeholder="Palabra detectada" value={source} onChange={e => setSource(e.target.value)} required /><span>→</span><input aria-label="Corrección" placeholder="Corrección" value={target} onChange={e => setTarget(e.target.value)} required /><button aria-label={editingRule?'Guardar corrección':'Agregar corrección'} disabled={busy}>{editingRule?<Check size={18}/>:<Plus size={18} />}</button></form>
          {editingRule&&<button className="text-link" onClick={()=>{setEditingRule(undefined);setSource('');setTarget('');}}>Cancelar edición</button>}
          <div className="table-head"><span>DETECTADO</span><span>REEMPLAZAR POR</span><span>ACTIVA</span></div>
          {rules.map(rule => <div className="rule-row" key={rule.id}><span>{rule.source}</span><strong>{rule.target}</strong><input type="checkbox" aria-label={`Activar ${rule.source}`} disabled={busy} checked={rule.enabled} onChange={e => run(() => api.saveRules(data.rules.map(r => r.id === rule.id ? { ...r, enabled: e.target.checked } : r), data.rules), "Regla actualizada")} /><div className="rule-tools"><button aria-label={`Editar ${rule.source}`} disabled={busy} onClick={()=>{setEditingRule(rule.id);setSource(rule.source);setTarget(rule.target);}}><Pencil size={15}/></button><button aria-label={`Eliminar ${rule.source}`} disabled={busy} onClick={() => run(() => api.saveRules(data.rules.filter(r => r.id !== rule.id), data.rules), "Regla eliminada")}><Trash2 size={15} /></button></div></div>)}
          {!rules.length && <div className="empty-state"><BookOpen size={25} /><h2>Sin correcciones</h2><p>Los nombres y términos guardados aparecerán aquí.</p></div>}
        </>}

        {route === "appearance" && <>
          <div className="artwork-picker" role="group" aria-label="Modelo de carpeta"><button className="artwork-arrow" aria-label="Modelo anterior" onClick={()=>changeArtwork(-1)}><ChevronLeft size={22}/></button><div className="appearance-preview">{data.settings.dictationArtwork==='metallic'?<IndicatorArtwork kind="folder" original={data.settings.metallicOriginal??true} color={data.settings.metallicColor??'#ffffff'}/>:<ControlledFolder color="black" customColor={data.settings.color} size="sm" visualState="rest" />}</div><button className="artwork-arrow" aria-label="Modelo siguiente" onClick={()=>changeArtwork(1)}><ChevronRight size={22}/></button></div><div className="artwork-model-name" aria-live="polite">{artworkModels[artworkIndex].name}<small>{artworkIndex+1} / {artworkModels.length}</small></div>
          <h3>Color de carpeta</h3><div className="form-row"><label>Color de carpeta<span>{data.settings.dictationArtwork==='metallic'?'Tinte metálico · conserva sombras y reflejos':'Superficie, bordes e iconos'}</span></label><PalettePanel value={{ base: "black", hex: appearanceColor }} onChange={value => patch(data.settings.dictationArtwork==='metallic'?{metallicColor:selectionHex(value),metallicOriginal:false}:{color:selectionHex(value)})} /></div>{data.settings.dictationArtwork==='metallic'&&<button className="text-link" onClick={()=>patch({metallicColor:'#ffffff',metallicOriginal:true})}>Restaurar color original</button>}<div className="color-values"><span style={{ background: appearanceColor }} /><code>{appearanceColor.toUpperCase()}</code></div>
            <h3>Carpeta de transcripción</h3><div className="form-row"><label htmlFor="pattern">Movimiento de los papeles</label><select id="pattern" disabled={data.settings.dictationArtwork==='metallic'} value={data.settings.pattern} onChange={e => patch({ pattern: e.target.value as api.Settings["pattern"] })}><option value="wave">Ola</option><option value="stairs">Escalera</option></select></div>
            <div className="form-row"><label htmlFor="recorder-scale">Tamaño de carpeta <span>{Math.round(data.settings.recorderScale*100)}%</span></label><div className="size-control"><input id="recorder-scale" type="range" min=".6" max="1.25" step=".05" value={data.settings.recorderScale} aria-valuetext={`${Math.round(data.settings.recorderScale*100)} %`} style={{'--range-progress':`${Math.max(0,Math.min(100,(data.settings.recorderScale-.6)/.65*100))}%`} as React.CSSProperties} onChange={e=>patch({recorderScale:Number(e.target.value)})}/><div className="size-control-limits"><span>Mín. 60 %</span><span>Máx. 125 %</span></div></div></div>
            <div className="form-row"><label htmlFor="placement">Posición de controles</label><select id="placement" value={data.settings.placement} onChange={e => patch({ placement: e.target.value as api.Settings["placement"] })}><option value="right">Derecha</option><option value="left">Izquierda</option><option value="top">Arriba</option><option value="bottom">Abajo</option></select></div>
            <h3>Indicadores</h3><div className="form-row"><label>Capturas y video<span>Cámara de cine · fondo transparente</span></label><div className="appearance-indicators"><IndicatorArtwork kind="camera"/></div></div>
            <h3>Icono de bandeja · Cristal</h3><div className="tray-samples"><span><img src="/cristal/16x16.png" width="16" height="16" alt="Icono 16 píxeles" />16 px</span><span><img src="/cristal/24x24.png" width="24" height="24" alt="Icono 24 píxeles" />24 px</span><span><img src="/cristal/32x32.png" width="32" height="32" alt="Icono 32 píxeles" />32 px</span></div>
        </>}

        {route === "hotkey" && <ShortcutSettings voice={data.settings.hotkey} onSaved={()=>void refresh()}/>}

        {route === "history" && <><div className="history-tabs" role="group" aria-label="Tipo de historial"><button aria-pressed={historyKind==='text'} onClick={()=>setHistoryKind('text')}>Transcripciones</button><button aria-pressed={historyKind==='captures'} onClick={()=>setHistoryKind('captures')}>Capturas y videos</button></div><div className="history-pane" hidden={historyKind!=='captures'}><CaptureHistory/></div><div className="history-pane" hidden={historyKind!=='text'}><div className="list-toolbar"><label className="search-field"><Search size={16} /><input aria-label="Buscar transcripción" placeholder="Buscar en transcripciones" value={search} onChange={e => setSearch(e.target.value)} /></label><span>{history.length} elementos</span></div>{history.slice(0,historyLimit).map(item => <button className="history-row" key={item.id} onClick={() => {setSelected(item);setMessage('');}}><time>{new Date(item.timestamp).toLocaleString("es-AR")}</time><span>{item.text}</span><ArrowUpRight size={16} /></button>)}{history.length>historyLimit&&<button className="history-more" onClick={()=>setHistoryLimit(n=>n+50)}>Mostrar 50 más</button>}{!history.length && <div className="empty-state"><History size={26} /><h2>{search?'Sin coincidencias':'Sin transcripciones'}</h2><p>{search?'Probá con otra palabra.':'Tus próximos dictados van a aparecer acá.'}</p></div>}</div></>}

        {route === "diagnostics" && <><div className="diagnostic-line"><span>Interfaz</span><strong>React + Motion</strong><Check size={16} /></div><div className="diagnostic-line"><span>Motor nativo</span><strong>{api.native ? "Tauri / Rust" : "No conectado"}</strong></div><div className="diagnostic-line"><span>Proveedor</span><strong>Groq</strong></div><h3>Eventos</h3><pre className="log-view">{data.logs.join("\n") || "Sin eventos en esta sesión."}</pre></>}
        </div>
      </section>
    </div>
    <ModalOverlay className="modal-backdrop desktop-app-modal" isOpen={!!selected} isDismissable onOpenChange={open => { if (!open) setSelected(undefined); }}><Modal className="transcript-modal"><Dialog aria-label="Transcripción">{selected && <><div className="modal-heading"><h2>Transcripción</h2><button aria-label="Cerrar transcripción" onClick={() => setSelected(undefined)}><X size={18} /></button></div><textarea aria-label="Texto de transcripción" value={selected.text} onChange={e => setSelected({ ...selected, text: e.target.value })} /><div className="transcript-edit-actions"><CopyButton text={selected.text} /><SaveButton busy={busy} onSave={()=>run(()=>api.saveTranscript(selected.id,selected.text),'Cambios guardados en el historial')}/></div>{message&&<p role="status">{message}</p>}</>}</Dialog></Modal></ModalOverlay>
  </div>;
}

function SettingsRoot(){
  const [windowError,setWindowError]=useState('');
  const [sidebarExpanded,setSidebarExpanded]=useState(()=>{
    try{const stored=localStorage.getItem('whispera.sidebar.expanded');return stored===null?window.innerWidth>640:stored==='true';}catch{return true;}
  });
  const toggleSidebar=()=>setSidebarExpanded(value=>{const next=!value;try{localStorage.setItem('whispera.sidebar.expanded',String(next));}catch{}return next;});
  return <div className="settings-window"><WindowChrome onError={setWindowError} sidebarExpanded={sidebarExpanded} onToggleSidebar={toggleSidebar}/>{windowError&&<p role="alert" className="window-control-error">{windowError}</p>}<SetupGate><SettingsApp sidebarExpanded={sidebarExpanded}/></SetupGate></div>;
}
const view=new URLSearchParams(location.search).get("view");
if(!view||view==='settings')document.documentElement.dataset.settingsShell='true';
if (view?.startsWith('screen-')) document.documentElement.dataset.screenSelect = 'true';
if (view === "record") document.documentElement.dataset.floating = "true";
if(api.native&&view!=='screen-freeze'){ const heartbeat=()=>void invoke('ui_heartbeat').catch(()=>{}); heartbeat();setInterval(heartbeat,2000); }
createRoot(document.getElementById("root")!).render(view === 'video-trim' ? <VideoTrim/> : view === 'screen-freeze' ? <ScreenFrozen/> : view === 'screen-select' ? <ScreenOverlay /> : view === 'screen-ink' ? <ScreenInk/> : view === 'screen-hud' ? <ScreenHud/> : view === 'screen-tools' ? <ScreenTools/> : view === 'screen-indicator' ? <ScreenIndicator /> : view === "import" ? <FileImport/> : view === "sounds" ? <SoundLab /> : view === "recorder" ? <RecorderPreview /> : view === "record" ? <FloatingRecorder /> : view === "details" ? <Recorder /> : <SettingsRoot/>);

import './plum-theme.css';
