import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { native } from './client';
import { type Settings } from './client';
import { MotionConfig } from 'motion/react';
import ControlledFolder from './ControlledFolder';
import { contrastInk } from './palette';
import './screen-recorder.css';
import { showWhenReady } from './screen-ready';
import HotkeyInput from './HotkeyInput';
import { finishHotkeyCapture } from './hotkey-capture';

type Preferences = { audio: 'none' | 'system' | 'microphone' | 'both'; hotkey: string; image_hotkey: string; frame_color: string };
type Appearance = Pick<Settings,'color'|'pattern'|'recorderScale'> & {frameColor:string};
type Status = { phase: string; seconds: number; path: string; error: string; copied: boolean };
type Point = { x: number; y: number };
type Recent = {id:string;kind:string;created_at:string;path:string};
type Rect = { x:number; y:number; width:number; height:number };
type Stage = { rect:Rect; kind:'video'|'image' };
export function ScreenOverlay() {
  const [kind,setKind] = useState<'video'|'image'>('video');
  const [stage,setStage] = useState<Stage>();
  const [epoch,setEpoch] = useState(0);
  const [initialized,setInitialized] = useState(false);
  const [frameColor,setFrameColor] = useState('#ffffff');
  useEffect(()=>{
    let alive=true; const off:UnlistenFn[]=[];
    const reset=async(mode:'video'|'image')=>{
      const appearance=await invoke<Appearance>('screen_appearance');
      if(!alive)return;setFrameColor(appearance.frameColor??'#ffffff');setStage(undefined);setKind(mode);setEpoch(v=>v+1);setInitialized(true);
      await showWhenReady('screen_overlay_ready',undefined,()=>alive);
    };
    void(async()=>{
      for(const promise of [listen('screen-hide',()=>{if(alive){setStage(undefined);setInitialized(false);}}),listen<'video'|'image'>('screen-reset',e=>void reset(e.payload)),listen<Stage>('screen-stage',e=>{if(alive)setStage(e.payload);})]){
        const remove=await promise;if(!alive)remove();else off.push(remove);
      }
      if(alive)await reset(await invoke<'video'|'image'>('screen_selection_kind'));
    })().catch(()=>{});
    return()=>{alive=false;off.forEach(remove=>remove());};
  },[]);
  return !initialized ? null : stage ? <ScreenIndicator rect={stage.rect} kind={stage.kind}/> : <ScreenSelection key={epoch} kind={kind} frameColor={frameColor} onPreparing={rect=>setStage({rect,kind})}/>;
}
export function ScreenSelection({kind='video',frameColor='#ffffff',onPreparing}:{kind?:'video'|'image';frameColor?:string;onPreparing?:(rect:Rect)=>void}={}) {
  const [origin, setOrigin] = useState<Point>();
  const [end, setEnd] = useState<Point>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const anchor = useRef<Point | undefined>(undefined);
  const cancel = () => invoke('screen_cancel_selection').catch(e => setError(String(e)));
  useEffect(() => {
    const listener = (e: KeyboardEvent) => {
      if (e.key === 'Escape') void cancel();
      if(e.ctrlKey && e.key.toLowerCase()==='z'){e.preventDefault();anchor.current=undefined;setOrigin(undefined);setEnd(undefined);}
      if(e.ctrlKey && e.key.toLowerCase()==='a'){
        e.preventDefault();void beginCapture({x:0,y:0,width:innerWidth,height:innerHeight});
      }
    };
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  }, []);
  const rect = origin && end ? { x: Math.min(origin.x, end.x), y: Math.min(origin.y, end.y), width: Math.abs(end.x - origin.x), height: Math.abs(end.y - origin.y) } : undefined;
  const beginCapture=async(rect:Rect)=>{
    if(busy)return;setBusy(true);onPreparing?.(rect);
    await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
    try{await invoke('screen_start',{rect});}catch(e){setError(String(e));setBusy(false);}
  };
  return <div className="screen-selection" data-has-selection={!!rect} onPointerDown={e => {
    if (busy || e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    anchor.current = { x: e.clientX, y: e.clientY }; setOrigin(anchor.current); setEnd(anchor.current); setError('');
  }} onPointerMove={e => { if (anchor.current && !busy) setEnd({ x: e.clientX, y: e.clientY }); }} onPointerUp={async e => {
    const start = anchor.current; anchor.current = undefined;
    if (!start || busy) return;
    const rect = { x: Math.min(start.x, e.clientX), y: Math.min(start.y, e.clientY), width: Math.abs(e.clientX - start.x), height: Math.abs(e.clientY - start.y) };
    if (rect.width < 16 || rect.height < 16) { setError('Seleccioná un área más grande.'); return; }
    await beginCapture(rect);
  }}>
    <div className="screen-selection-help" role="status">{busy ? 'Preparando…' : error || (kind==='image'?'Seleccioná el área para capturar y editar.':'Seleccioná el área. Al soltar empieza a grabar.')} <kbd>Ctrl+A: pantalla completa</kbd><kbd>Esc: salir</kbd></div>
    {rect && <div className="screen-selection-rect" style={{ left: rect.x, top: rect.y, width: rect.width, height: rect.height, borderColor:frameColor }}><span>{Math.round(rect.width)} × {Math.round(rect.height)}</span></div>}
  </div>;
}

export function ScreenIndicator({rect:area,kind='video'}:{rect?:Rect;kind?:'video'|'image'}={}) {
  const query = new URLSearchParams(location.search);
  const rect = area ?? { x: Number(query.get('x')), y: Number(query.get('y')), width: Number(query.get('width')), height: Number(query.get('height')) };
  const [status, setStatus] = useState<Status>({ phase: 'starting', seconds: 0, path: '', error: '', copied: false });
  const [settings, setSettings] = useState<Appearance>();
  const [step, setStep] = useState(0);
  useEffect(() => {
    let alive = true; let timer: ReturnType<typeof setTimeout>;
    void invoke<Appearance>('screen_appearance').then(data => { if (alive) setSettings(data); }).catch(() => {});
    const poll = async () => {
      try { const value = await invoke<Status>('screen_status'); if (alive) setStatus(value); }
      catch { /* The native state remains authoritative; a transient poll failure must not stop capture. */ }
      finally { if (alive) timer = setTimeout(poll, 250); }
    };
    if (native) void poll();
    return () => { alive = false; clearTimeout(timer); };
  }, []);
  useEffect(() => {
    if (!['recording', 'saving'].includes(status.phase)) return;
    const timer = setInterval(() => setStep(value => value + 1), status.phase === 'saving' ? 1200 : 650);
    return () => clearInterval(timer);
  }, [status.phase]);
  // The animated cards extend above the folder; reserve their full visual area.
  const scale = Math.min(settings?.recorderScale ?? .85, (innerHeight - 16) / 450, (innerWidth - 32) / 321);
  const width = 321 * scale; const height = 270 * scale;
  const right = rect.x + rect.width + 12;
  const left = right + width <= innerWidth - 8 ? right : rect.x - width - 12 >= 8 ? rect.x - width - 12 : Math.max(8, innerWidth - width - 8);
  const top = Math.max(8 + 180 * scale, Math.min(rect.y, innerHeight - height - 8));
  const color = settings?.color ?? '#9024DC';
  const label = status.error ? 'Revisá Configuración' : status.phase === 'saving' ? 'Preparando video' : status.phase === 'recording' ? 'Grabando video' : status.phase==='starting'||status.phase==='selecting' ? 'Iniciando…' : status.copied ? 'Video copiado' : 'Video listo';
  const time = `${Math.floor(status.seconds / 60).toString().padStart(2, '0')}:${Math.floor(status.seconds % 60).toString().padStart(2, '0')}`;
  return <div className="screen-indicator" aria-label="Indicador de grabación de pantalla">
    {['selecting','starting','editing','recording'].includes(status.phase) && <div className="screen-recording-frame" data-image={kind==='image'} data-testid="recording-frame" style={{ left: rect.x, top: rect.y, width: rect.width, height: rect.height, borderColor: settings?.frameColor??'#ffffff' }} />}
    {kind==='video'&&<div className="screen-folder-indicator" style={{ left, top, width, height }}><MotionConfig reducedMotion="user"><div style={{ width: 321, height: 270, transform: `scale(${scale})`, transformOrigin: 'top left' }}>
      <ControlledFolder color="black" customColor={color} size="md" pattern={settings?.pattern ?? 'wave'} visualState={status.phase === 'saving' ? (step % 2 ? 'open' : 'rest') : 'hover'} recordingStep={status.phase === 'recording' ? step : undefined} />
      <div className="screen-folder-caption" style={{ color: contrastInk(color) }}><span role="status"><i />{label}</span><output aria-label="Tiempo grabado">{time}</output></div>
    </div></MotionConfig></div>}
  </div>;
}

export default function ScreenRecorder() {
  const [preferences, setPreferences] = useState<Preferences>({ audio: 'none', hotkey: 'Control+Shift+F9', image_hotkey:'Control+Shift+F10', frame_color:'#ffffff' });
  const [status, setStatus] = useState<Status>({ phase: 'idle', seconds: 0, path: '', error: '', copied: false });
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [recent,setRecent]=useState<Recent[]>([]);
  const [recentOpen,setRecentOpen]=useState(false);
  useEffect(()=>{
    if(!native||!recentOpen)return;let alive=true;
    void invoke<Recent[]>('screen_recent').then(rows=>{if(alive)setRecent(rows);}).catch(e=>{if(alive)setMessage(String(e));});
    return()=>{alive=false;};
  },[recentOpen,status.phase]);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    invoke<Preferences>('screen_preferences').then(p => { if (alive) setPreferences({...p,frame_color:p.frame_color??'#ffffff'}); }).catch(e => { if (alive) setMessage(String(e)); });
    let timer:ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try { if(document.visibilityState==='visible'){const s=await invoke<Status>('screen_status');if(alive)setStatus(s);} }
      catch(e){if(alive)setMessage(String(e));}
      finally{if(alive)timer=setTimeout(refresh,500);}
    };
    void refresh();
    return () => { alive = false; clearInterval(timer); };
  }, []);
  const run = async (action: () => Promise<unknown>, success = '') => {
    setBusy(true); setMessage('');
    try { await action(); setMessage(success); } catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  };
  const occupied = ['recording', 'saving', 'selecting', 'starting', 'editing'].includes(status.phase);
  return <section className="screen-recorder-panel">
    <>
      <h3>Audio predeterminado</h3>
      <div className="form-row"><label htmlFor="screen-audio">Sonido del video<span>Se usa en todas las grabaciones de pantalla.</span></label><select id="screen-audio" disabled={occupied || busy} value={preferences.audio} onChange={e => setPreferences({ ...preferences, audio: e.target.value as Preferences['audio'] })}><option value="none">Sin audio</option><option value="system">Audio de la computadora</option><option value="microphone">Micrófono</option><option value="both">Computadora y micrófono</option></select></div>
      <div className="form-row"><label htmlFor="screen-hotkey">Atajo de pantalla<span>El mismo atajo inicia y detiene.</span></label><HotkeyInput id="screen-hotkey" disabled={occupied || busy} value={preferences.hotkey} onChange={hotkey => setPreferences({ ...preferences, hotkey })} /></div>
      <div className="form-row"><label htmlFor="image-hotkey">Atajo de captura de imagen<span>Seleccioná, marcá y copiá. Independiente del video.</span></label><HotkeyInput id="image-hotkey" disabled={occupied || busy} value={preferences.image_hotkey} onChange={image_hotkey=>setPreferences({...preferences,image_hotkey})}/></div>
      <div className="form-row"><label htmlFor="screen-frame-color">Color del recuadro<span>Para capturas y video. Blanco por defecto.</span></label><input id="screen-frame-color" type="color" value={preferences.frame_color} disabled={occupied||busy} onChange={e=>setPreferences({...preferences,frame_color:e.target.value})}/></div>
      <button disabled={!native || busy || occupied} onClick={() => void run(async () => { await finishHotkeyCapture(); await invoke('screen_save_preferences', { preferences }); }, 'Preferencias de pantalla guardadas.')}>Guardar preferencias</button>
      <p className="muted-note">Seleccioná el área y grabá. Al detener, el video queda en el portapapeles para pegar con Ctrl+V en aplicaciones que admitan archivos. Se conserva una copia temporal en esta computadora.</p>
    </>
    <div className="screen-record-actions">
      <button disabled={!native || busy || occupied} onClick={() => void run(() => invoke('screen_select'))}>Seleccionar área y grabar</button>
      <button disabled={!native||busy||occupied} onClick={()=>void run(()=>invoke('screen_select_image'))}>Capturar imagen</button>
      {status.phase === 'recording' && <p role="status">Grabando. Pulsá otra vez tu atajo para terminar.</p>}
      {status.phase === 'selecting' && <p role="status">Seleccioná el área. Escape cancela.</p>}
      {status.phase === 'saving' && <p role="status">Preparando video para pegar…</p>}
    </div>
    <p className="capture-shortcuts">Deshacer <kbd>Ctrl+Z</kbd> · Rehacer <kbd>Ctrl+Shift+Z</kbd> · Alinear trazos <kbd>Shift</kbd></p>
    <details className="capture-recent" onToggle={e=>setRecentOpen(e.currentTarget.open)}><summary>Capturas recientes <span>Hasta 12 imágenes y videos</span></summary>
      {recentOpen&&<div className="capture-recent-list">{recent.length?recent.map(item=><div key={item.id} className="capture-recent-row"><span className="capture-file-kind">{item.kind==='image'?'PNG':'MP4'}</span><div><strong>{item.kind==='image'?'Captura de imagen':'Grabación de pantalla'}</strong><time>{new Date(item.created_at).toLocaleString('es-AR')}</time></div><button disabled={busy||occupied} onClick={()=>void run(()=>invoke('screen_recent_copy',{id:item.id}),'Copiado. Pegalo con Ctrl+V.')}>Copiar</button><button disabled={busy} title="Mostrar archivo" aria-label="Mostrar archivo" onClick={()=>void run(()=>invoke('screen_recent_reveal',{id:item.id}))}>↗</button></div>):<p className="muted-note">Las próximas capturas que copies o guardes aparecen acá.</p>}</div>}
    </details>
    {status.path && !occupied && <div className="screen-result"><p>{status.copied ? 'Video copiado. Pegalo con Ctrl+V.' : 'Video disponible. Podés volver a copiarlo.'}</p><button onClick={() => void run(() => invoke('screen_copy'), 'Video copiado para pegar.')}>Copiar video</button><button onClick={() => void run(() => invoke('screen_reveal'))}>Ver archivo temporal</button></div>}
    {(message || status.error) && <p role="status" className="notice">{message || status.error}</p>}
    {!native && <p>La captura de pantalla requiere la aplicación de Windows.</p>}
  </section>;
}
