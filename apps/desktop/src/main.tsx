import React, { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Dialog, Modal, ModalOverlay } from "react-aria-components";
import { AudioLines, BookOpen, Palette, Keyboard, History, Activity, ArrowUpRight, Search, Plus, Trash2, Check, Upload, X, Paintbrush, Download } from "lucide-react";
import { CopyButton, SaveButton, SectionReveal, SettingsSwitch } from "./ResourceControls";
import { Button } from "../vendor/components/ui/button";
import "@fontsource-variable/instrument-sans";
import ControlledFolder from "./ControlledFolder";
import PalettePanel from "./PalettePanel";
import RecorderPreview from "./RecorderPreview";
import Recorder from "./Recorder";
import FloatingRecorder from "./FloatingRecorder";
import FileImport from './FileImport';
import SoundLab, { soundPairs } from "./SoundLab";
import { Pencil, Volume2, RotateCcw, Play } from 'lucide-react';
import { invoke } from "@tauri-apps/api/core";
import { selectionHex } from "./palette";
import * as api from "./client";
import "./style.css";
import "./desktop.css";
import Onboarding, { SetupGate } from './Onboarding';

const routes = [
  { id: "transcription", name: "Transcripción", icon: AudioLines, group: "Preferencias" },
  { id: "dictionary", name: "Diccionario personal", icon: BookOpen },
  { id: "appearance", name: "Apariencia", icon: Palette },
  { id: "color", name: "Personalizado", icon: Paintbrush },
  { id: "hotkey", name: "Atajo de grabación", icon: Keyboard },
  { id: "sounds", name: "Sonidos", icon: Volume2 },
  { id: "history", name: "Historial", icon: History, group: "Tu espacio" },
  { id: "diagnostics", name: "Diagnóstico", icon: Activity },
] as const;
type Route = typeof routes[number]["id"];
const descriptions: Record<Route, string> = {
  transcription: "Tu voz, con tus preferencias.", dictionary: "Las palabras que tienen que salir bien.",
  appearance: "Una carpeta a tu manera.", color: "Encontrá tu color.",
  hotkey: "Tu próximo dictado, a una tecla.", history: "Todo lo que dijiste, en un lugar.",
  diagnostics: "El estado de Whispera.",
  sounds: "Inicio y fin del dictado.",
};

function SettingsApp() {
  const [setup, setSetup] = useState(false);
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
  const refresh = () => api.snapshot().then(setData);
  useEffect(() => { refresh().catch(e => setMessage(String(e))); }, []);
  const run = async (work: () => Promise<unknown>, success: string) => { setBusy(true); try { await work(); await refresh(); setMessage(success); return true; } catch (error) { setMessage(String(error)); return false; } finally { setBusy(false); } };
  if (!data) return <div className="desktop-loading" role="status">{message || "Cargando configuración…"}</div>;
  if (setup) return <Onboarding onDone={() => { setSetup(false); void refresh(); }} />;
  const patch = (change: Partial<api.Settings>) => setData({ ...data, settings: { ...data.settings, ...change } });
  const save = () => run(() => api.saveSettings(data.settings), api.native ? "Configuración guardada" : "Borrador de vista previa guardado");
  const rules = data.rules.filter(r => `${r.source} ${r.target}`.toLowerCase().includes(search.toLowerCase()));
  const history = data.history.filter(r => r.text.toLowerCase().includes(search.toLowerCase()));
  const title = routes.find(r => r.id === route)!.name;

  return <div className="desktop-app" style={{ "--accent": data.settings.color } as React.CSSProperties}>
    <aside className="desktop-sidebar">
      <button onClick={() => setSetup(true)}>Guía inicial / Setup</button>
      <a className="desktop-brand" href="?view=settings"><span className="brand-mark"><img src="/cristal/128x128.png" alt="" /></span><span className="brand-copy"><strong>Whispera</strong><small>Tu espacio de voz</small></span></a>
      <nav aria-label="Configuración">{routes.map(item => <React.Fragment key={item.id}>{"group" in item && <p className="nav-group">{item.group}</p>}<button aria-label={item.name} title={item.name} aria-current={route === item.id ? "page" : undefined} className={route === item.id ? "active" : ""} onClick={() => { setRoute(item.id); setSearch(""); setMessage(""); }}><item.icon size={17} /><span>{item.name}</span>{item.id === "dictionary" && <small>{data.rules.length}</small>}</button></React.Fragment>)}</nav>
      <div className="sidebar-bottom"><a className="recorder-link" href="?view=record" onClick={e=>{if(api.native){e.preventDefault();void run(()=>invoke('open_recorder'),'Grabadora abierta');}}}><AudioLines size={18} /><span>Abrir grabadora</span><ArrowUpRight size={15} /></a><span className="engine-status"><i />{api.native ? "Whispera 2" : "Vista previa"}<small>Groq</small></span></div>
    </aside>
    <div className="desktop-main">
      <section className="desktop-content">
        <div className="page-heading"><div><h1>{title}</h1><p>{descriptions[route]}</p></div>{["transcription", "appearance", "color", "hotkey", "sounds", "diagnostics"].includes(route) && <SaveButton key={route} busy={busy} onSave={save} />}</div>
        {message && <div className="notice" role="status">{message}<button aria-label="Cerrar aviso" onClick={() => setMessage("")}><X size={14} /></button></div>}
        <SectionReveal key={route}>
        {route==='sounds'&&<><div className="form-row"><label htmlFor="sounds-on">Sonidos de grabación</label><SettingsSwitch id="sounds-on" label="Activar sonidos" checked={data.settings.sounds} onChange={v=>patch({sounds:v})}/></div><div className="form-row"><label htmlFor="sound-theme">Inicio y fin</label><select id="sound-theme" value={data.settings.soundTheme} onChange={e=>patch({soundTheme:e.target.value})}>{soundPairs.map(p=><option value={p.id} key={p.id}>{p.name}</option>)}</select></div><div className="page-actions">{(['start','stop'] as const).map(cue=><button key={cue} onClick={()=>{const audio=new Audio(`/sound-lab/${data.settings.soundTheme}-${cue}.wav`);audio.volume=.35;void audio.play().catch(e=>setMessage(String(e)));}}><Play size={15}/>{cue==='start'?'Escuchar inicio':'Escuchar fin'}</button>)}</div></>}
        {route==='diagnostics'&&<><div className="form-row"><label htmlFor="watchdog">Recuperar interfaz sin respuesta</label><SettingsSwitch id="watchdog" label="Vigilancia de interfaz" checked={data.settings.watchdog} onChange={v=>patch({watchdog:v})}/></div><button disabled={!api.native||busy} onClick={()=>run(()=>invoke('restart_app'),'Reiniciando')}><RotateCcw size={16}/>Reiniciar Whispera</button></>}
        {route === "transcription" && <>
          <div className="provider-line"><div className="provider-logo"><AudioLines size={22} /></div><div><h2>Groq</h2><p>Proveedor de transcripción</p></div><span className="state-tag"><i />{data.keyConfigured ? "Clave guardada" : "Sin conectar"}</span></div>
          <h3>Voz e idioma</h3>
          <div className="form-row"><label htmlFor="model">Modelo</label><select id="model" value={data.settings.model} onChange={e => patch({ model: e.target.value })}><option value="whisper-large-v3-turbo">Whisper Large v3 Turbo</option><option value="whisper-large-v3">Whisper Large v3</option></select></div>
          <div className="form-row"><label htmlFor="language">Idioma del audio</label><select id="language" value={data.settings.language} onChange={e => patch({ language: e.target.value })}><option value="es">Español</option><option value="en">English</option><option value="pt">Português</option><option value="auto">Detectar automáticamente</option></select></div>
          <h3>Conexión</h3><div className="key-line"><label htmlFor="key">Clave API de Groq <span>Almacenada en Windows</span></label><div><input id="key" type="password" autoComplete="off" value={key} placeholder={data.keyConfigured ? "••••••••••••••••" : "gsk_…"} onChange={e => setKey(e.target.value)} /><Button variant="secondary" size="lg" disabled={busy || !key || !api.native} onClick={() => run(async () => { await api.saveKey(key); setKey(""); }, "Clave guardada en el almacén de Windows")}>Guardar clave</Button></div></div>

          <h3>Al terminar</h3><div className="form-row"><label htmlFor="copy">Copiar automáticamente<span>El texto queda en tu portapapeles.</span></label><SettingsSwitch id="copy" label="Copiar al finalizar" checked={data.settings.autoCopy} onChange={checked => patch({ autoCopy: checked })} /></div>
          <div className="form-row"><label htmlFor="paste">Pegar en el destino original</label><SettingsSwitch id="paste" label="Pegar al finalizar dictado" checked={data.settings.autoPaste} onChange={checked=>patch({autoPaste:checked})}/></div>
          <div className="form-row"><label htmlFor="trim">Reducir silencios en el envío</label><SettingsSwitch id="trim" label="Recortar silencios" checked={data.settings.trimSilence} onChange={checked=>patch({trimSilence:checked})}/></div>
          <div className="page-actions"><Button variant="secondary" size="lg" disabled={busy || !api.native} onClick={() => run(()=>invoke('open_import'), "Ventana de audio abierta") }><Upload data-icon="inline-start" />Transcribir archivo</Button></div>
        </>}

        {route === "dictionary" && <>
          <div className="list-toolbar"><label className="search-field"><Search size={16} /><input aria-label="Buscar corrección" placeholder="Buscar palabra o sustitución" value={search} onChange={e => setSearch(e.target.value)} /></label><span>{data.rules.length} reglas</span></div>
          <form className="dictionary-add" onSubmit={e => { e.preventDefault(); if (source.trim() && target.trim()) run(async () => { const updated={id:editingRule??crypto.randomUUID(),source:source.trim(),target:target.trim(),enabled:data.rules.find(r=>r.id===editingRule)?.enabled??true}; await api.saveRules(editingRule?data.rules.map(r=>r.id===editingRule?updated:r):[...data.rules,updated]); setSource(""); setTarget(""); setEditingRule(undefined); }, "Corrección guardada"); }}><input aria-label="Palabra detectada" placeholder="Palabra detectada" value={source} onChange={e => setSource(e.target.value)} required /><span>→</span><input aria-label="Corrección" placeholder="Corrección" value={target} onChange={e => setTarget(e.target.value)} required /><button aria-label={editingRule?'Guardar corrección':'Agregar corrección'} disabled={busy}>{editingRule?<Check size={18}/>:<Plus size={18} />}</button></form>
          {editingRule&&<button className="text-link" onClick={()=>{setEditingRule(undefined);setSource('');setTarget('');}}>Cancelar edición</button>}
          <div className="table-head"><span>DETECTADO</span><span>REEMPLAZAR POR</span><span>ACTIVA</span></div>
          {rules.map(rule => <div className="rule-row" key={rule.id}><span>{rule.source}</span><strong>{rule.target}</strong><input type="checkbox" aria-label={`Activar ${rule.source}`} checked={rule.enabled} onChange={e => run(() => api.saveRules(data.rules.map(r => r.id === rule.id ? { ...r, enabled: e.target.checked } : r)), "Regla actualizada")} /><div className="rule-tools"><button aria-label={`Editar ${rule.source}`} onClick={()=>{setEditingRule(rule.id);setSource(rule.source);setTarget(rule.target);}}><Pencil size={15}/></button><button aria-label={`Eliminar ${rule.source}`} onClick={() => run(() => api.saveRules(data.rules.filter(r => r.id !== rule.id)), "Regla eliminada")}><Trash2 size={15} /></button></div></div>)}
          {!rules.length && <div className="empty-state"><BookOpen size={25} /><h2>Sin correcciones</h2><p>Los nombres y términos guardados aparecerán aquí.</p></div>}
        </>}

        {(route === "appearance" || route === "color") && <>
          <div className="appearance-preview"><ControlledFolder color="black" customColor={data.settings.color} size="sm" visualState="rest" /></div>
          {route === "color" ? <><h3>Paleta personal</h3><div className="form-row"><label>Color de carpeta<span>Superficie, bordes e iconos</span></label><PalettePanel value={{ base: "black", hex: data.settings.color }} onChange={value => patch({ color: selectionHex(value) })} /></div><div className="color-values"><span style={{ background: data.settings.color }} /><code>{data.settings.color.toUpperCase()}</code></div></> : <>
            <h3>Comportamiento visual</h3><div className="form-row"><label htmlFor="pattern">Movimiento de los papeles</label><select id="pattern" value={data.settings.pattern} onChange={e => patch({ pattern: e.target.value as api.Settings["pattern"] })}><option value="wave">Ola</option><option value="stairs">Escalera</option></select></div>
            <div className="form-row"><label htmlFor="recorder-scale">Tamaño de grabadora <span>{Math.round(data.settings.recorderScale*100)}%</span></label><input id="recorder-scale" type="range" min=".6" max="1.25" step=".05" value={data.settings.recorderScale} onChange={e=>patch({recorderScale:Number(e.target.value)})}/></div>
            <div className="form-row"><label htmlFor="placement">Posición de controles</label><select id="placement" value={data.settings.placement} onChange={e => patch({ placement: e.target.value as api.Settings["placement"] })}><option value="right">Derecha</option><option value="left">Izquierda</option><option value="top">Arriba</option><option value="bottom">Abajo</option></select></div>
            <button className="text-link" onClick={() => setRoute("color")}><Paintbrush size={16} />Personalizar color <ArrowUpRight size={15} /></button>
            <h3>Icono de bandeja · Cristal</h3><div className="tray-samples"><span><img src="/cristal/16x16.png" width="16" height="16" alt="Icono 16 píxeles" />16 px</span><span><img src="/cristal/24x24.png" width="24" height="24" alt="Icono 24 píxeles" />24 px</span><span><img src="/cristal/32x32.png" width="32" height="32" alt="Icono 32 píxeles" />32 px</span></div>
          </>}
        </>}

        {route === "hotkey" && <><div className="shortcut-display"><Keyboard size={24} /><kbd>{data.settings.hotkey}</kbd></div><h3>Acceso global</h3><div className="form-row"><label htmlFor="hotkey">Atajo de grabación<span>Iniciar y detener desde otras aplicaciones</span></label><input id="hotkey" value={data.settings.hotkey} onChange={e => patch({ hotkey: e.target.value })} /></div><p className="muted-note">La activación global se verificará en la aplicación nativa.</p></>}

        {route === "history" && <><div className="list-toolbar"><label className="search-field"><Search size={16} /><input aria-label="Buscar transcripción" placeholder="Buscar en transcripciones" value={search} onChange={e => setSearch(e.target.value)} /></label><span>{data.history.length} elementos</span></div>{history.map(item => <button className="history-row" key={item.id} onClick={() => setSelected(item)}><time>{new Date(item.timestamp).toLocaleString("es-AR")}</time><span>{item.text}</span><ArrowUpRight size={16} /></button>)}{!history.length && <div className="empty-state"><History size={26} /><h2>Sin transcripciones</h2><p>Tu historial anterior no se modifica.</p></div>}</>}

        {route === "diagnostics" && <><div className="diagnostic-line"><span>Interfaz</span><strong>React + Motion</strong><Check size={16} /></div><div className="diagnostic-line"><span>Motor nativo</span><strong>{api.native ? "Tauri / Rust" : "No conectado"}</strong></div><div className="diagnostic-line"><span>Proveedor</span><strong>Groq</strong></div><h3>Eventos</h3><pre className="log-view">{data.logs.join("\n") || "Sin eventos en esta sesión."}</pre></>}
        </SectionReveal>
      </section>
    </div>
    <ModalOverlay className="modal-backdrop desktop-app-modal" isOpen={!!selected} isDismissable onOpenChange={open => { if (!open) setSelected(undefined); }}><Modal className="transcript-modal"><Dialog aria-label="Transcripción">{selected && <><div className="modal-heading"><h2>Transcripción</h2><button aria-label="Cerrar transcripción" onClick={() => setSelected(undefined)}><X size={18} /></button></div><textarea aria-label="Texto de transcripción" value={selected.text} onChange={e => setSelected({ ...selected, text: e.target.value })} /><CopyButton text={selected.text} /></>}</Dialog></Modal></ModalOverlay>
  </div>;
}

const view=new URLSearchParams(location.search).get("view");
if (view === "record") document.documentElement.dataset.floating = "true";
if(api.native){ const heartbeat=()=>void invoke('ui_heartbeat').catch(()=>{}); heartbeat();setInterval(heartbeat,2000); }
createRoot(document.getElementById("root")!).render(view === "import" ? <FileImport/> : view === "sounds" ? <SoundLab /> : view === "recorder" ? <RecorderPreview /> : view === "record" ? <FloatingRecorder /> : view === "details" ? <Recorder /> : <SetupGate><SettingsApp /></SetupGate>);
