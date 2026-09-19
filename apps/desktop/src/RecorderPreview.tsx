import React, { useEffect, useState } from "react";
import { MotionConfig } from "motion/react";
import { Check, Mic, RotateCcw, Square } from "lucide-react";
import OriginalFolder from "../vendor/components/ui/folder-component";
import ControlledFolder from "./ControlledFolder";
import FolderControls, { type Placement } from "./FolderControls";
import PalettePanel from "./PalettePanel";
import { loadSelection } from "./palette";
import "./style.css";

type Phase = "idle" | "recording" | "processing" | "done";
type Color = "black" | "white" | "blue";
const labels: Record<Phase, string> = {
  idle: "En espera", recording: "Grabando", processing: "Procesando", done: "Completado",
};

export default function RecorderPreview() {
  const [mode, setMode] = useState<"demo" | "original">("demo");
  const [phase, setPhase] = useState<Phase>("idle");
  const [selection, setSelection] = useState(loadSelection);
  const color = selection.base;
  const setColor = (base: Color) => setSelection({ base });
  useEffect(() => {
    try { localStorage.setItem("whispera-folder-demo-color", JSON.stringify(selection)); } catch { /* Optional persistence. */ }
  }, [selection]);
  const [background, setBackground] = useState("dark");
  const [seconds, setSeconds] = useState(0);
  const [paused, setPaused] = useState(false);
  const [muted, setMuted] = useState(false);
  const [placement, setPlacement] = useState<Placement>("right");
  const [pattern, setPattern] = useState<"stairs" | "wave">("wave");
  const [step, setStep] = useState(0);
  const [controlsOpen, setControlsOpen] = useState(false);
  const [small, setSmall] = useState(window.innerWidth < 480);
  const [reduceMotion, setReduceMotion] = useState(window.matchMedia("(prefers-reduced-motion: reduce)").matches);

  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduceMotion(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    const query = window.matchMedia("(max-width: 479px)");
    const update = () => setSmall(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    if (phase !== "recording" || paused) return;
    const id = window.setInterval(() => setSeconds(value => value + 1), 1000);
    return () => window.clearInterval(id);
  }, [phase, paused]);

  useEffect(() => {
    if (phase !== "recording" || paused || reduceMotion) return;
    const id = window.setInterval(() => setStep(value => value + 1), 650);
    return () => window.clearInterval(id);
  }, [phase, paused, reduceMotion, pattern]);

  useEffect(() => {
    if (phase !== "recording") { setControlsOpen(false); return; }
    const id = window.setTimeout(() => setControlsOpen(true), 160);
    return () => window.clearTimeout(id);
  }, [phase]);

  const reset = () => { setPhase("idle"); setSeconds(0); setStep(0); setPaused(false); setMuted(false); };
  const start = () => { setSeconds(0); setStep(0); setPaused(false); setPhase("recording"); };
  const stop = () => { setPaused(false); setMuted(false); setPhase("processing"); };
  const state = phase === "recording" ? "hover" : "rest";
  const time = `${Math.floor(seconds / 60).toString().padStart(2, "0")}:${(seconds % 60).toString().padStart(2, "0")}`;

  return (
    <main data-phase={phase} data-visual-state={state} data-step={step}>
      <header>
        <div className="brand">Whispera <span>DEMO</span></div>
        <div className="segmented" aria-label="Modo">
          <button aria-pressed={mode === "demo"} onClick={() => { reset(); setMode("demo"); }}>Simulación</button>
          <button aria-pressed={mode === "original"} onClick={() => { reset(); setMode("original"); }}>Original</button>
        </div>
      </header>
      {mode === "demo" && <nav className="prototype-options" aria-label="Prototipos">
        <div className="segmented" aria-label="Ubicación de controles">{([['right', 'Derecha'], ['top', 'Arriba'], ['left', 'Izquierda'], ['bottom', 'Abajo']] as const).map(([value, label]) =>
          <button key={value} aria-pressed={placement === value} onClick={() => { reset(); setPlacement(value); }}>{label}</button>)}</div>
        <select aria-label="Animación de grabación" value={pattern} onChange={e => { setPattern(e.target.value as "stairs" | "wave"); setStep(0); }}><option value="stairs">01 · Escalera</option><option value="wave">02 · Ola</option></select>
      </nav>}
      <section className="workspace" data-background={background} aria-label="Carpeta">
        <div className="scene" data-testid="scene" data-layout={placement}>
          <MotionConfig reducedMotion={reduceMotion ? "always" : "never"}>
            {mode === "original"
              ? <OriginalFolder color={color} size={small ? "sm" : "md"} />
              : <div className="folder-rig"><ControlledFolder color={color} customColor={selection.hex} size="md" visualState={state} recordingStep={phase === "recording" && !reduceMotion ? step : undefined} pattern={pattern} />
                  <FolderControls color={color} customColor={selection.hex} paused={paused} muted={muted} open={controlsOpen && phase === "recording"} placement={placement}
                    onMute={() => setMuted(value => !value)} onPause={() => setPaused(value => !value)} onStop={stop} />
                </div>}
          </MotionConfig>
        </div>
        <div className="transport">
          <div className="readout">
            <span className="status" data-status={phase} role="status"><i />{mode === "original" ? "Original" : paused ? "En pausa" : labels[phase]}{muted && mode === "demo" ? " · Silenciado" : ""}</span>
            <output aria-label="Tiempo simulado">{time}</output>
          </div>
          <div className="actions">
            {mode === "demo" && <>
              {(phase === "idle" || phase === "done") && <button className="primary" onClick={start}><Mic size={18} />Simular grabación</button>}
              {phase === "recording" && <button className="primary" onClick={stop}><Square size={16} fill="currentColor" />Detener</button>}
              {phase === "processing" && <button className="primary" onClick={() => setPhase("done")}><Check size={19} />Finalizar prueba</button>}
              <button className="icon-button" aria-label="Reiniciar" title="Reiniciar" onClick={reset}><RotateCcw size={18} /></button>
            </>}
          </div>
        </div>
      </section>
      <footer>
        {mode === "demo" ? <PalettePanel value={selection} onChange={setSelection} /> : <div className="options"><span>Carpeta</span><div className="swatches" aria-label="Color de carpeta">
          {(["black", "white", "blue"] as const).map((value, index) => <button key={value} className="swatch" style={{ background: ["#070707", "#ffffff", "#50b1fd"][index] }} aria-label={["Negro", "Blanco", "Azul"][index]} title={["Negro", "Blanco", "Azul"][index]} aria-pressed={color === value} onClick={() => setColor(value)} />)}
        </div></div>}
        <div className="options"><label htmlFor="surface">Fondo</label><select id="surface" value={background} onChange={e => setBackground(e.target.value)}><option value="dark">Oscuro</option><option value="black">Negro</option><option value="light">Claro</option></select></div>
      </footer>
    </main>
  );
}
