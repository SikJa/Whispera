import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { listenCapture as listen } from './capture-events';
import { native } from './client';
import { type Settings } from './client';
import './screen-recorder.css';
import { showWhenReady } from './screen-ready';
import CaptureFrame from './CaptureFrame';
import EditableCaptureFrame,{type CaptureContext} from './EditableCaptureFrame';
import { SettingsSwitch } from './ResourceControls';
import { type CapturePreferences as Preferences, defaultCapturePreferences, VideoAudioSettings } from './CaptureSettings';

type Appearance = Pick<Settings,'color'|'pattern'|'recorderScale'> & {frameColor:string};
type Status = { phase: string; seconds: number; path: string; error: string; copied: boolean };
type Point = { x: number; y: number };
type Rect = { x:number; y:number; width:number; height:number };
type Stage = { rect:Rect; kind:'video'|'image' };
export function ScreenOverlay() {
  const [kind,setKind] = useState<'video'|'image'>('video');
  const [stage,setStage] = useState<Stage>();
  const [editor,setEditor]=useState<CaptureContext>();
  const [epoch,setEpoch] = useState(0);
  const [initialized,setInitialized] = useState(false);
  const [frameColor,setFrameColor] = useState('#ffffff');
  const [snapshot,setSnapshot] = useState<string>();
  useEffect(()=>{
    let alive=true; let imageUrl:string|undefined; let resetId=0; const off:UnlistenFn[]=[];
    const reset=async(mode:'video'|'image')=>{
      const id=++resetId;
      const [appearance,bytes]=await Promise.all([
        invoke<Appearance>('screen_appearance'),
        mode==='image'?invoke<ArrayBuffer>('screen_selection_image'):Promise.resolve(undefined),
      ]);
      if(!alive||id!==resetId)return;
      if(imageUrl)URL.revokeObjectURL(imageUrl);
      imageUrl=bytes&&bytes.byteLength?URL.createObjectURL(new Blob([bytes],{type:'image/bmp'})):undefined;
      if(imageUrl){const image=new Image();image.src=imageUrl;await image.decode();}
      if(!alive||id!==resetId)return;
      setSnapshot(imageUrl);setFrameColor(appearance.frameColor??'#ffffff');setStage(undefined);setEditor(undefined);setKind(mode);setEpoch(v=>v+1);setInitialized(true);
      await showWhenReady('screen_overlay_ready',undefined,()=>alive&&id===resetId);
    };
    void(async()=>{
      for(const promise of [listen('screen-hide',()=>{++resetId;if(imageUrl)URL.revokeObjectURL(imageUrl);imageUrl=undefined;if(alive){setSnapshot(undefined);setStage(undefined);setEditor(undefined);setInitialized(false);}}),listen<'video'|'image'>('screen-reset',e=>void reset(e.payload)),listen<Stage>('screen-stage',e=>{if(alive)setStage(e.payload);}),listen<CaptureContext|null>('screen-editor-reset',e=>{if(alive){setEditor(e.payload??undefined);if(e.payload&&imageUrl){URL.revokeObjectURL(imageUrl);imageUrl=undefined;setSnapshot(undefined);}}})]){
        const remove=await promise;if(!alive)remove();else off.push(remove);
      }
      // Preloaded windows only register listeners; no hidden rendering/polling loop.
      if(alive&&(await invoke<Status>('screen_status')).phase==='selecting')
        await reset(await invoke<'video'|'image'>('screen_selection_kind'));
    })().catch(()=>{});
    return()=>{alive=false;if(imageUrl)URL.revokeObjectURL(imageUrl);off.forEach(remove=>remove());};
  },[]);
  return !initialized ? null : editor?.rect ? <EditableCaptureFrame context={editor}/> : stage ? <ScreenIndicator rect={stage.rect} kind={stage.kind}/> : <ScreenSelection key={epoch} kind={kind} frameColor={frameColor} snapshot={snapshot} onPreparing={rect=>setStage({rect,kind})}/>;
}
export function ScreenSelection({kind='video',frameColor='#ffffff',snapshot,onPreparing}:{kind?:'video'|'image';frameColor?:string;snapshot?:string;onPreparing?:(rect:Rect)=>void}={}) {
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
  return <div className="screen-selection" style={snapshot?{backgroundImage:`url("${snapshot}")`,backgroundSize:'100% 100%'}:undefined} data-frozen={!!snapshot} data-has-selection={!!rect} onPointerDown={e => {
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
    <div className="screen-selection-help" role="status">{busy ? 'Preparando…' : error || (kind==='image'?'Seleccioná el área para capturar.':'Seleccioná el área. Al soltar empieza a grabar.')} <kbd>Ctrl+A: pantalla completa</kbd><kbd>Esc: salir</kbd></div>
    {rect && <CaptureFrame className="screen-selection-rect" width={rect.width} height={rect.height} color={frameColor} style={{left:rect.x,top:rect.y}}/>}
  </div>;
}

export function ScreenIndicator({rect:area,kind='video'}:{rect?:Rect;kind?:'video'|'image'}={}) {
  const query = new URLSearchParams(location.search);
  const rect = area ?? { x:Number(query.get('x')),y:Number(query.get('y')),width:Number(query.get('width')),height:Number(query.get('height')) };
  const [color,setColor]=useState('#ffffff');
  useEffect(()=>{let alive=true;void invoke<Appearance>('screen_appearance').then(settings=>{if(alive)setColor(settings.frameColor??'#ffffff');}).catch(()=>{});return()=>{alive=false;};},[]);
  return <div className="screen-indicator" aria-label="Área de captura"><CaptureFrame className="screen-recording-frame" image={kind==='image'} testId="recording-frame" width={rect.width} height={rect.height} color={color} style={{left:rect.x,top:rect.y}}/></div>;
}

export default function ScreenRecorder() {
  const [windowsShortcuts, setWindowsShortcuts] = useState(false);
  useEffect(()=>{if(native)void invoke<boolean>('windows_capture_shortcuts').then(setWindowsShortcuts).catch(()=>{});},[]);
  const [preferences, setPreferences] = useState<Preferences>(defaultCapturePreferences);
  const [status, setStatus] = useState<Status>({ phase: 'idle', seconds: 0, path: '', error: '', copied: false });
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    invoke<Preferences>('screen_preferences').then(p => { if (alive) setPreferences({...defaultCapturePreferences,...p}); }).catch(e => { if (alive) setMessage(String(e)); });
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
  const occupied = ['recording', 'paused', 'pausing', 'resuming', 'reframing', 'saving', 'cancelling', 'selecting', 'starting', 'editing'].includes(status.phase);
  return <section className="screen-recorder-panel">
    <>
      <h3>Audio predeterminado</h3>
      <VideoAudioSettings value={preferences.audio} disabled={occupied||busy} onChange={audio=>setPreferences(p=>({...p,audio}))}/>
      <h3>Capturas de imagen</h3>
      <div className="form-row"><label htmlFor="windows-capture-shortcuts">Impr Pant para Whispera<span>Mientras Whispera está abierto, las capturas de Windows pasan a Ctrl + Alt + Shift + F12.</span></label><SettingsSwitch id="windows-capture-shortcuts" label="Impr Pant para Whispera" checked={windowsShortcuts} disabled={!native||occupied||busy} onChange={enabled=>void run(async()=>{await invoke('save_windows_capture_shortcuts',{enabled});setWindowsShortcuts(enabled);},'Atajos de captura guardados.')}/></div>
      <div className="form-row"><label htmlFor="image-auto-copy">Copiar al soltar la selección<span>Va directo al portapapeles, sin abrir el editor.</span></label><SettingsSwitch id="image-auto-copy" label="Copiar al soltar la selección" checked={preferences.image_auto_copy} disabled={occupied||busy} onChange={image_auto_copy=>setPreferences(p=>({...p,image_auto_copy}))}/></div>
      <div className="form-row"><label htmlFor="screen-frame-color">Color del recuadro<span>Para capturas y video. Blanco por defecto.</span></label><input id="screen-frame-color" type="color" value={preferences.frame_color} disabled={occupied||busy} onChange={e=>setPreferences({...preferences,frame_color:e.target.value})}/></div>
      <button disabled={!native || busy || occupied} onClick={() => void run(async () => { const saved=await invoke<Preferences>('screen_preferences'); await invoke('screen_save_preferences', { preferences:{...saved,audio:preferences.audio,frame_color:preferences.frame_color,image_auto_copy:preferences.image_auto_copy} }); }, 'Preferencias de pantalla guardadas.')}>Guardar preferencias</button>
      <p className="muted-note">Seleccioná el área y grabá. Al detener, el video queda en el portapapeles para pegar con Ctrl+V en aplicaciones que admitan archivos. Se conserva una copia temporal en esta computadora.</p>
    </>
    <div className="screen-record-actions">
      <button disabled={!native || busy || occupied} onClick={() => void run(() => invoke('screen_select'))}>Seleccionar área y grabar</button>
      <button disabled={!native||busy||(occupied&&!['recording','paused'].includes(status.phase))} onClick={()=>void run(()=>invoke('screen_select_image'))}>Capturar imagen</button>
      {status.phase === 'recording' && <p role="status">Grabando. Pulsá otra vez tu atajo para terminar.</p>}
      {status.phase === 'selecting' && <p role="status">Seleccioná el área. Escape cancela.</p>}
      {status.phase === 'saving' && <p role="status">Preparando video para pegar…</p>}
    </div>
    <p className="capture-shortcuts">Deshacer <kbd>Ctrl+Z</kbd> · Rehacer <kbd>Ctrl+Shift+Z</kbd> · Alinear trazos <kbd>Shift</kbd></p>

    {status.path && !occupied && <div className="screen-result"><p>{status.copied ? 'Video copiado. Pegalo con Ctrl+V.' : 'Video disponible. Podés volver a copiarlo.'}</p><button onClick={() => void run(() => invoke('screen_copy'), 'Video copiado para pegar.')}>Copiar video</button><button onClick={() => void run(() => invoke('screen_reveal'))}>Ver archivo temporal</button></div>}
    {(message || status.error) && <p role="status" className="notice">{message || status.error}</p>}
    {!native && <p>La captura de pantalla requiere la aplicación de Windows.</p>}
  </section>;
}
