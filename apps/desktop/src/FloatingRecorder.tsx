import { useEffect, useState, type CSSProperties, type MouseEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MotionConfig } from 'motion/react';
import { Mic, Copy, Check, Pencil, Settings2, X, FolderOpen, AlertCircle } from 'lucide-react';
import ControlledFolder from './ControlledFolder';
import FolderControls from './FolderControls';
import { native, snapshot, type Settings } from './client';
import { contrastInk } from './palette';
import './floating-recorder.css';

type State = { phase: string; seconds: number; error: string; text: string; muted: boolean; progress: string };
const labels: Record<string, string> = { idle: 'Listo', recording: 'Grabando', paused: 'En pausa', processing: 'Transcribiendo', ready: 'Audio guardado', done: 'Transcripción lista', error: 'Revisar audio' };
export default function FloatingRecorder() {
  const [state, setState] = useState<State>({ phase: 'idle', seconds: 0, error: '', text: '', muted: false, progress: '' });
  const [settings, setSettings] = useState<Settings>();
  const [step, setStep] = useState(0);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const scale = settings?.recorderScale ?? .85;
  useEffect(() => { if(native) void invoke('recorder_size',{scale}).catch(e=>setError(String(e))); },[scale]);
  useEffect(() => {
    if(!native)return;
    let pending=false; let disposed=false; let previous='';
    const timer=setInterval(async()=>{
      if(pending||disposed)return;
      const rects=Array.from(document.querySelectorAll('[data-hit], [data-slot="folder-card"], .glass-control:not([disabled]), .float-actions button, .float-error, .float-cancel'))
        .map(el=>el.getBoundingClientRect()).filter(r=>r.width>0&&r.height>0).map(r=>({x:r.x,y:r.y,width:r.width,height:r.height}));
      const signature=JSON.stringify(rects.map(r=>[r.x,r.y,r.width,r.height].map(Math.round)));
      if(signature===previous||!rects.length)return; pending=true;
      try{await invoke('recorder_region',{rects});previous=signature;}catch(e){if(!disposed)setError(String(e));}finally{pending=false;}
    },50);
    return()=>{disposed=true;clearInterval(timer);};
  },[]);
  const recording = ['recording', 'paused'].includes(state.phase);
  useEffect(() => {
    let disposed = false; let timer: ReturnType<typeof setTimeout>; let refreshed = 0;
    async function poll() {
      try {
        if (native) { const value = await invoke<State>('recording_state'); if (!disposed) setState(value); }
        if (Date.now() - refreshed > 1500) { const value = await snapshot(); if (!disposed) setSettings(value.settings); refreshed = Date.now(); }
      } catch (e) { if (!disposed) setError(String(e)); }
      finally { if (!disposed) timer = setTimeout(poll, 250); }
    }
    void poll(); return () => { disposed = true; clearTimeout(timer); };
  }, []);
  useEffect(() => {
    if (state.phase !== 'recording' && state.phase !== 'processing') return;
    const timer = setInterval(() => setStep(s => s + 1), state.phase === 'processing' ? 1200 : 650);
    return () => clearInterval(timer);
  }, [state.phase]);
  useEffect(() => {
    if (!recording) { setOpen(false); return; }
    const timer = setTimeout(() => setOpen(true), 160); return () => clearTimeout(timer);
  }, [recording]);
  useEffect(() => setCopied(false), [state.text]);
  async function run(command: string, args?: Record<string, unknown>) {
    if (busy) return false;
    setBusy(true); setError('');
    try { await invoke(command, args); return true; }
    catch (e) { setError(String(e)); return false; }
    finally { setBusy(false); }
  }
  function drag(e: MouseEvent) {
    if (!native || e.button !== 0 || (e.target as Element).closest('button, a, input')) return;
    e.preventDefault(); void getCurrentWindow().startDragging().catch(e => setError(String(e)));
  }
  const color = settings?.color ?? '#9024DC';
  const visual = recording ? 'hover' : state.phase === 'processing' ? (step % 2 ? 'open' : 'rest') : 'rest';
  const time = `${Math.floor(state.seconds / 60).toString().padStart(2, '0')}:${Math.floor(state.seconds % 60).toString().padStart(2, '0')}`;
  return <main className="floating-recorder" data-phase={state.phase} aria-label="Grabadora flotante" style={{ transform: `scale(${scale})`, transformOrigin: 'top left', '--folder-color': color, '--folder-ink': contrastInk(color) } as CSSProperties}>
    <MotionConfig reducedMotion="user"><div className="float-rig" onMouseDown={drag}>
      <ControlledFolder color="black" customColor={color} visualState={visual} recordingStep={recording ? step : undefined} pattern={settings?.pattern ?? 'wave'} size="md" />
      <FolderControls color="black" customColor={color} open={open} placement={settings?.placement ?? 'right'} muted={state.muted} paused={state.phase === 'paused'} onMute={() => void run('recording_action', { action: 'mute' })} onPause={() => void run('recording_action', { action: 'pause' })} onStop={() => void run('recording_action', { action: 'stop' })} />
      <div className="float-readout"><span role="status" title={state.progress || labels[state.phase]}><i data-phase={state.phase} />{labels[state.phase]}</span><output aria-label="Tiempo grabado">{time}</output></div>
      <div className="float-actions">
        {!recording && state.phase !== 'processing' && <button aria-label="Grabar" title="Grabar" disabled={busy || !native} onClick={() => run('recording_action', { action: 'start' })}><Mic /></button>}
        {state.phase === 'done' && <button aria-label="Copiar transcripción" title={copied ? 'Copiado' : 'Copiar transcripción'} onClick={async () => { if (await run('copy_recording')) setCopied(true); }}>{copied ? <Check /> : <Copy />}</button>}
        <button aria-label={state.phase === 'done' ? 'Editar transcripción' : 'Transcribir audio'} title={state.phase === 'done' ? 'Editar transcripción' : 'Transcribir audio'} disabled={!native} onClick={() => run(state.phase === 'done' ? 'open_recording_details' : 'open_import')}>{state.phase === 'done' ? <Pencil /> : <FolderOpen />}</button>
        <button aria-label="Configuración" title="Configuración" disabled={!native} onClick={() => run('open_settings')}><Settings2 /></button>
        <button aria-label={recording ? 'Cancelar grabación' : 'Ocultar grabadora'} title={recording ? 'Cancelar grabación' : 'Ocultar grabadora'} disabled={!native} onClick={() => recording ? setCancelling(true) : getCurrentWindow().hide()}><X /></button>
      </div>
      {cancelling && recording && <div className="float-cancel"><span>¿Cancelar sin transcribir?</span><button onClick={async()=>{if(await run('recording_action',{action:'cancel'}))setCancelling(false);}}>Cancelar grabación</button><button onClick={()=>setCancelling(false)}>Seguir grabando</button></div>}
      {(error || state.error) && <button className="float-error" role="alert" title={error || state.error} onClick={() => run('open_recording_details')}><AlertCircle size={14} /><span>{error || state.error}</span></button>}
    </div></MotionConfig>
  </main>;
}
