import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {UnlistenFn} from '@tauri-apps/api/event';
import {Monitor} from 'lucide-react';
import type {Settings} from './client';
import {listenCapture as listen} from './capture-events';
import {showWhenReady} from './screen-ready';
import CaptureFrame from './CaptureFrame';
import EditableCaptureFrame,{type CaptureContext} from './EditableCaptureFrame';
import {commandKey,isMac} from './platform';
import './screen-recorder.css';

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
      try {
      const [appearance,bytes]=await Promise.all([
        invoke<Appearance>('screen_appearance'),
        // Windows already renders ScreenFrozen below this transparent selector.
        // Native readiness waits for that layer before showing us. Avoid a second
        // full-monitor transfer/decode; retain the existing macOS path unchanged.
        mode==='image'&&isMac?invoke<ArrayBuffer>('screen_selection_image'):Promise.resolve(undefined),
      ]);
      if(!alive||id!==resetId)return;
      if(imageUrl)URL.revokeObjectURL(imageUrl);
      imageUrl=bytes&&bytes.byteLength?URL.createObjectURL(new Blob([bytes],{type:'image/bmp'})):undefined;
      if(imageUrl){const image=new Image();image.src=imageUrl;await image.decode();}
      if(!alive||id!==resetId)return;
      setSnapshot(imageUrl);setFrameColor(appearance.frameColor??'#ffffff');setStage(undefined);setEditor(undefined);setKind(mode);setEpoch(v=>v+1);setInitialized(true);
      await showWhenReady('screen_overlay_ready',undefined,()=>alive&&id===resetId);
      } catch (error) {
        if(!alive||id!==resetId)return;
        if(imageUrl)URL.revokeObjectURL(imageUrl);
        imageUrl=undefined;setSnapshot(undefined);setInitialized(false);
        console.error('Capture selector initialization failed',error);
        await invoke('screen_cancel_selection').catch(()=>{});
      }
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
  return !initialized ? null : editor?.rect ? <EditableCaptureFrame context={editor}/> : stage ? <ScreenIndicator rect={stage.rect} kind={stage.kind}/> : <ScreenSelection key={epoch} kind={kind} frameColor={frameColor} snapshot={snapshot}/>;
}
export function ScreenSelection({kind='video',frameColor='#ffffff',snapshot,onPreparing}:{kind?:'video'|'image';frameColor?:string;snapshot?:string;onPreparing?:(rect:Rect)=>void}={}) {
  const [origin, setOrigin] = useState<Point>();
  const [end, setEnd] = useState<Point>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [monitors,setMonitors]=useState<{index:number;name:string|null;width:number;height:number}[]>([]);
  useEffect(()=>{let alive=true;if(kind==='video')void invoke<typeof monitors>('screen_monitors').then(value=>{if(alive&&Array.isArray(value))setMonitors(value);}).catch(e=>{if(alive)setError(String(e));});return()=>{alive=false;};},[kind]);
  const anchor = useRef<Point | undefined>(undefined);
  const boundedPoint=(x:number,y:number):Point=>({x:Math.max(0,Math.min(innerWidth,x)),y:Math.max(0,Math.min(innerHeight,y))});
  const interruptSelection=()=>{
    if(!anchor.current)return;
    anchor.current=undefined;setOrigin(undefined);setEnd(undefined);
  };
  const cancel = () => invoke('screen_cancel_selection').catch(e => setError(String(e)));
  useEffect(() => {
    const listener = (e: KeyboardEvent) => {
      if (e.key === 'Escape') void cancel();
      if((e.ctrlKey || (isMac && e.metaKey)) && e.key.toLowerCase()==='z'){e.preventDefault();anchor.current=undefined;setOrigin(undefined);setEnd(undefined);}
      if((e.ctrlKey || (isMac && e.metaKey)) && e.key.toLowerCase()==='a'){
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
    anchor.current = boundedPoint(e.clientX,e.clientY); setOrigin(anchor.current); setEnd(anchor.current); setError('');
  }} onPointerMove={e => { if (anchor.current && !busy) setEnd(boundedPoint(e.clientX,e.clientY)); }} onPointerUp={async e => {
    const start = anchor.current; anchor.current = undefined;
    if (!start || busy) return;
    const end=boundedPoint(e.clientX,e.clientY);
    const rect = { x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y) };
    if (rect.width < 16 || rect.height < 16) { setError('Seleccioná un área más grande.'); return; }
    await beginCapture(rect);
  }} onPointerCancel={interruptSelection} onLostPointerCapture={interruptSelection}>
    <div className="screen-selection-help" role="status">{busy ? 'Preparando…' : error || (kind==='image'?'Seleccioná el área para capturar.':'Seleccioná el área. Al soltar empieza a grabar.')} <kbd>{commandKey}+A: pantalla completa</kbd><kbd>Esc: salir</kbd></div>
    {kind==='video'&&!origin&&<div className="screen-monitor-picker" role="group" aria-label="Pantalla para grabar">{monitors.map(m=><button key={m.index} disabled={busy} title={`${m.name??'Monitor'} · ${m.width} × ${m.height}`} onClick={()=>{if(busy)return;setBusy(true);void invoke('screen_start_monitor',{index:m.index}).catch(e=>{setError(String(e));setBusy(false);});}}><Monitor size={18}/><span>Pantalla {m.index+1}<small>{m.width} × {m.height}</small></span></button>)}</div>}
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

