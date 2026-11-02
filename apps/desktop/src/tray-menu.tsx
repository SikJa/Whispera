// Tray menu window: a light entry (React + icons only) so it opens instantly.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mic, Video, ScanLine, ClipboardList, FileAudio, Settings, RefreshCw, Power, ArrowUpCircle, Headphones, Pause, Play, Square, Download, X, Activity } from "lucide-react";
import "@fontsource-variable/instrument-sans";
import { applyAppTheme } from "./app-theme-data";
import "./tray-menu.css";

type Keys = string[];
type State = {
  version: string; phase: string; videoBusy: boolean; update: string | null; theme: string; background: string;
  shortcuts: { recorder: Keys; video: Keys; capture: Keys; library: Keys; reading?: Keys };
  reader?: {supported:boolean;phase:string;exporting:boolean;has_reading:boolean};
};
const native = "__TAURI_INTERNALS__" in window;
const preview: State = {
  version: "0.2.35", phase: "idle", videoBusy: false, update: null, theme: "plum", background: "",
  shortcuts: { recorder: ["Ctrl", "Shift", "Espacio"], video: ["Alt", "X"], capture: ["Alt", "A"], library: ["Alt", "C"] },
};

function Keycaps({ keys, compact }: { keys: Keys; compact?: boolean }) {
  if (!keys.length) return null;
  // Tiles are narrow: the Shift symbol keeps long combos on one line.
  const shown = compact ? keys.map(key => key === "Shift" ? "⇧" : key) : keys;
  return <span className="tm-keys" title={keys.join(" + ")}>{shown.map((key, i) => <kbd key={i}>{key}</kbd>)}</span>;
}

function status(state: State) {
  if (state.phase === "recording") return { tone: "live", text: "Grabando" };
  if (state.phase === "paused") return { tone: "warn", text: "En pausa" };
  if (state.phase === "processing") return { tone: "busy", text: "Transcribiendo" };
  if (state.videoBusy) return { tone: "live", text: "Video en curso" };
  return { tone: "ready", text: "Listo" };
}

function Row({ icon, label, hint, keys, action, run, tone, disabled }: { icon: ReactNode; label: string; hint?: ReactNode; keys?: Keys; action: string; run: (a: string) => void; tone?: string; disabled?:boolean }) {
  return <button className="tm-row" disabled={disabled} data-item data-tone={tone} onClick={() => run(action)}>
    <span className="tm-row-icon">{icon}</span>
    <span className="tm-row-label">{label}</span>
    {hint}
    {keys && <Keycaps keys={keys} />}
  </button>;
}

function TrayMenu() {
  const [state, setState] = useState<State>(preview);
  const [opening, setOpening] = useState(0);
  const card = useRef<HTMLDivElement>(null);
  const load = useCallback(async () => {
    if (!native) return;
    try { const next = await invoke<State>("tray_menu_state"); setState(next); applyAppTheme(next.theme, next.background); } catch { /* keep last state */ }
  }, []);
  const run = useCallback((action: string) => {
    if (native) void invoke("tray_menu_action", { action }).catch(() => {});
  }, []);
  useEffect(() => {
    try { applyAppTheme(localStorage.getItem("whispera.app.theme") ?? "plum", localStorage.getItem("whispera.app.background") ?? ""); } catch { applyAppTheme("plum"); }
    void load();
    if (!native) return;
    const off = [listen("tray-menu-open", () => { setOpening(n => n + 1); void load(); }),listen("reader-tray-state",()=>void load())];
    return () => { for(const pending of off)void pending.then(stop => stop()); };
  }, [load]);
  // The native window hugs the card so nothing transparent catches clicks.
  useLayoutEffect(() => {
    const element = card.current; if (!element || !native) return;
    const fit = () => void invoke("tray_menu_fit", { height: Math.ceil(element.scrollHeight) + 20 }).catch(() => {});
    fit(); const observer = new ResizeObserver(fit); observer.observe(element);
    return () => observer.disconnect();
  }, []);
  // Each opening takes keyboard focus without highlighting an action; arrows start at the first.
  const shell = useRef<HTMLDivElement>(null);
  useEffect(() => { shell.current?.focus({ preventScroll: true }); }, [opening]);
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); run("close"); return; }
    const items = Array.from(card.current?.querySelectorAll<HTMLButtonElement>("[data-item]:not(:disabled)") ?? []);
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    const step = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : e.key === "ArrowUp" || e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    items[(index + step + items.length) % items.length]?.focus();
  };
  const now = status(state);
  return <div ref={shell} tabIndex={-1} className="tm-shell" onKeyDown={onKey}>
    <div ref={card} key={opening} className="tm-card" role="menu" aria-label="Whispera">
      <header className="tm-head">
        <img className="tm-logo" src="/cristal/64x64.png" alt="" />
        <div className="tm-title"><strong>Whispera</strong><span>Versión {state.version}</span></div>
        <span className="tm-status" data-tone={now.tone}><i />{now.text}</span>
      </header>
      <div className="tm-tiles">
        <button className="tm-tile" data-item data-live={state.phase === "recording"} onClick={() => run("recorder")}><span className="tm-tile-icon"><Mic size={20} strokeWidth={1.5} /></span><span className="tm-tile-label">Dictado</span><Keycaps compact keys={state.shortcuts.recorder} /></button>
        <button className="tm-tile" data-item data-live={state.videoBusy} onClick={() => run("screen")}><span className="tm-tile-icon"><Video size={20} strokeWidth={1.5} /></span><span className="tm-tile-label">Video</span><Keycaps compact keys={state.shortcuts.video} /></button>
        <button className="tm-tile" data-item onClick={() => run("screenshot")}><span className="tm-tile-icon"><ScanLine size={20} strokeWidth={1.5} /></span><span className="tm-tile-label">Captura</span><Keycaps compact keys={state.shortcuts.capture} /></button>
      </div>
      <div className="tm-group">
        <Row icon={<ClipboardList size={16} strokeWidth={1.5} />} label="Portapapeles" keys={state.shortcuts.library} action="library" run={run} />
        <Row icon={<FileAudio size={16} strokeWidth={1.5} />} label="Transcribir archivo…" action="import" run={run} />
      </div>
      {state.reader?.supported&&<div className="tm-group" aria-label="Controles de lectura">
        <Row icon={<Headphones size={16} strokeWidth={1.5}/>} label="Lectura en voz alta" keys={state.shortcuts.reading} action="reading" run={run}/>
        <Row icon={state.reader.phase==='paused'?<Play size={16}/>:<Pause size={16}/>} label={state.reader.phase==='paused'?'Continuar lectura':'Pausar lectura'} disabled={!['speaking','paused'].includes(state.reader.phase)} action="reader-pause" run={run}/>
        <Row icon={<Square size={16}/>} label="Detener lectura" disabled={!['speaking','paused'].includes(state.reader.phase)} action="reader-stop" run={run}/>
        <Row icon={<Download size={16}/>} label={state.reader.exporting?'Guardando MP3…':'Guardar lectura como MP3…'} disabled={!state.reader.has_reading||state.reader.exporting} action="reader-export" run={run}/>
        {state.reader.exporting&&<Row icon={<X size={16}/>} label="Cancelar guardado MP3" action="reader-export-cancel" run={run}/>}
      </div>}
      <div className="tm-group">
        <Row icon={<Activity size={16} strokeWidth={1.5}/>} label="Uso de IA" action="ai-usage" run={run}/>
        <Row icon={<Settings size={16} strokeWidth={1.5} />} label="Configuración" action="settings" run={run} />
        {state.update
          ? <Row icon={<ArrowUpCircle size={16} strokeWidth={1.5} />} label="Actualizar Whispera" hint={<span className="tm-update">{state.update}</span>} action="updates" run={run} tone="accent" />
          : <Row icon={<RefreshCw size={16} strokeWidth={1.5} />} label="Buscar actualizaciones" action="updates" run={run} />}
      </div>
      <div className="tm-group tm-foot">
        <Row icon={<Power size={16} strokeWidth={1.5} />} label="Salir de Whispera" action="quit" run={run} tone="danger" />
      </div>
    </div>
  </div>;
}

createRoot(document.getElementById("root")!).render(<TrayMenu />);
