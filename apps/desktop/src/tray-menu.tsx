// Tray menu window: a light entry (React + icons only) so it opens instantly.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mic, Video, ScanLine, ClipboardList, FileAudio, Settings, RefreshCw, Power, ArrowUpCircle } from "lucide-react";
import "@fontsource-variable/instrument-sans";
import { applyAppTheme } from "./app-theme-data";
import "./tray-menu.css";

type Keys = string[];
// One window, two menus: everything for the tray icon, a short one for the floating folder.
type Mode = "tray" | "folder";
const asMode = (value: unknown): Mode | null => value === "tray" || value === "folder" ? value : null;
type State = {
  version: string; mode?: string; phase: string; videoBusy: boolean; update: string | null; theme: string; background: string;
  shortcuts: { recorder: Keys; video: Keys; capture: Keys; library: Keys };
};
const native = "__TAURI_INTERNALS__" in window;
const t = (es: string, en: string) => navigator.language.startsWith("es") ? es : en;
const preview: State = {
  version: "0.2.35", phase: "idle", videoBusy: false, update: null, theme: "plum", background: "",
  shortcuts: { recorder: ["Ctrl", "Shift", "Espacio"], video: ["Alt", "X"], capture: ["Alt", "A"], library: ["Alt", "C"] },
};

function Keycaps({ keys, compact }: { keys: Keys; compact?: boolean }) {
  if (!keys.length) return null;
  // Tiles are narrow: the Shift symbol and a short Space keep long combos on one line.
  const short: Record<string, string> = { Shift: "⇧", Espacio: "Esp" };
  const shown = compact ? keys.map(key => short[key] ?? key) : keys;
  return <span className="tm-keys" title={keys.join(" + ")}>{shown.map((key, i) => <kbd key={i}>{key}</kbd>)}</span>;
}

function status(state: State) {
  if (state.phase === "recording") return { tone: "live", text: "Grabando" };
  if (state.phase === "paused") return { tone: "warn", text: "En pausa" };
  if (state.phase === "processing") return { tone: "busy", text: "Transcribiendo" };
  if (state.videoBusy) return { tone: "live", text: "Video en curso" };
  return { tone: "ready", text: "Listo" };
}

function Row({ icon, label, hint, keys, action, run, tone }: { icon: ReactNode; label: string; hint?: ReactNode; keys?: Keys; action: string; run: (a: string) => void; tone?: string }) {
  return <button className="tm-row" data-item data-tone={tone} onClick={() => run(action)}>
    <span className="tm-row-icon">{icon}</span>
    <span className="tm-row-label">{label}</span>
    {hint}
    {keys && <Keycaps keys={keys} />}
  </button>;
}

function TrayMenu() {
  const [state, setState] = useState<State>(preview);
  const [opening, setOpening] = useState(0);
  // Empty while closed: the next opening never shows the previous menu for a frame.
  const [mode, setMode] = useState<Mode | null>(native ? null : "tray");
  const card = useRef<HTMLDivElement>(null);
  const load = useCallback(async () => {
    if (!native) return;
    try {
      const next = await invoke<State>("tray_menu_state"); setState(next); applyAppTheme(next.theme, next.background);
      // A page that finished loading after the open event still learns which menu it is.
      const known = asMode(next.mode); if (known) setMode(current => current ?? known);
    } catch { /* keep last state */ }
  }, []);
  const run = useCallback((action: string) => {
    if (native) void invoke("tray_menu_action", { action }).catch(() => {});
  }, []);
  useEffect(() => {
    try { applyAppTheme(localStorage.getItem("whispera.app.theme") ?? "plum", localStorage.getItem("whispera.app.background") ?? ""); } catch { applyAppTheme("plum"); }
    void load();
    if (!native) return;
    const opened = listen<string>("tray-menu-open", event => { setMode(asMode(event.payload) ?? "tray"); setOpening(n => n + 1); void load(); });
    const closed = listen("tray-menu-close", () => flushSync(() => setMode(null)));
    return () => { void opened.then(stop => stop()); void closed.then(stop => stop()); };
  }, [load]);
  // The native window hugs the card so nothing transparent catches clicks.
  // Each opening draws a new card, so the observer follows it.
  useLayoutEffect(() => {
    const element = card.current; if (!element || !native || !mode) return;
    const fit = () => void invoke("tray_menu_fit", { height: Math.ceil(element.getBoundingClientRect().height + 20), mode }).catch(() => {});
    fit(); const observer = new ResizeObserver(fit); observer.observe(element);
    return () => observer.disconnect();
  }, [opening, mode]);
  // Each opening takes keyboard focus without highlighting an action; arrows start at the first.
  const shell = useRef<HTMLDivElement>(null);
  useEffect(() => { shell.current?.focus({ preventScroll: true }); }, [opening]);
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); run("close"); return; }
    const items = Array.from(card.current?.querySelectorAll<HTMLButtonElement>("[data-item]") ?? []);
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    const step = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : e.key === "ArrowUp" || e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    items[(index + step + items.length) % items.length]?.focus();
  };
  const now = status(state);
  return <div ref={shell} tabIndex={-1} className="tm-shell" onKeyDown={onKey} onContextMenu={e => e.preventDefault()}>
    {mode === "folder" && <div ref={card} key={opening} className="tm-card" data-mode="folder" role="menu" aria-label={t("Carpeta flotante", "Floating folder")}>
      <div className="tm-group">
        <Row icon={<ClipboardList size={16} strokeWidth={1.5} />} label={t("Portapapeles", "Clipboard")} keys={state.shortcuts.library} action="library" run={run} />
        <Row icon={<Settings size={16} strokeWidth={1.5} />} label={t("Configuración", "Settings")} action="settings" run={run} />
      </div>
    </div>}
    {mode === "tray" && <div ref={card} key={opening} className="tm-card" role="menu" aria-label="Whispera">
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
      <div className="tm-group">
        <Row icon={<Settings size={16} strokeWidth={1.5} />} label="Configuración" action="settings" run={run} />
        {state.update
          ? <Row icon={<ArrowUpCircle size={16} strokeWidth={1.5} />} label="Actualizar Whispera" hint={<span className="tm-update">{state.update}</span>} action="updates" run={run} tone="accent" />
          : <Row icon={<RefreshCw size={16} strokeWidth={1.5} />} label="Buscar actualizaciones" action="updates" run={run} />}
      </div>
      <div className="tm-group tm-foot">
        <Row icon={<Power size={16} strokeWidth={1.5} />} label="Salir de Whispera" action="quit" run={run} tone="danger" />
      </div>
    </div>}
  </div>;
}

createRoot(document.getElementById("root")!).render(<TrayMenu />);
