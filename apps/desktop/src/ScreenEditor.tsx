import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { PaletteContents } from './PalettePanel';
import { selectionHex } from './palette';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ArrowUpRight, Camera, ChevronLeft, ChevronRight, Circle, Triangle, Diamond, Hexagon, Star, MoreHorizontal, SlidersHorizontal, Pause, Play, Copy, Highlighter, Minus, MousePointer2, Pencil, Printer, Redo2, RotateCcw, Save, Square, SquareDashed, Trash2, Type, Undo2, X } from 'lucide-react';
import { blurredTile, bounds, commit, constrained, emptyHistory, hitTest, isShape, paint, redo, undo, type Shape, type History, type Mark, type Point, type Tool } from './screen-annotations';
import './screen-recorder.css';
import './capture-line.css';
import { showWhenReady } from './screen-ready';
import CaptureFrame from './CaptureFrame';
import type {CaptureContext} from './EditableCaptureFrame';

type Context = CaptureContext;
type Action = { action: string; value?: string };
type Feedback = { tool: Tool; color: string; width: number; canUndo: boolean; canRedo: boolean; count: number; busy: boolean; error: string };
const defaults: Feedback = { tool: 'pointer', color: '#ff4545', width: 3, canUndo: false, canRedo: false, count: 0, busy: false, error: '' };
const tools = [
  ['pointer', MousePointer2, 'Puntero / seleccionar', 'V'], ['pen', Pencil, 'Lápiz', 'P'], ['line', Minus, 'Línea', 'L'],
  ['arrow', ArrowUpRight, 'Flecha', 'A'], ['rectangle', Square, 'Recuadro', 'R'], ['highlight', Highlighter, 'Resaltador', 'H'],
  ['text', Type, 'Texto', 'T'], ['blur', SquareDashed, 'Bloque difuminado', 'B'],
] as const;
const shapes = [['rectangle',Square,'Rectángulo'],['ellipse',Circle,'Círculo / elipse'],['triangle',Triangle,'Triángulo'],['diamond',Diamond,'Rombo'],['hexagon',Hexagon,'Hexágono'],['star',Star,'Estrella']] as const;
function preferences() {
  try { const p = JSON.parse(localStorage.getItem('whispera.ink.v1') ?? '{}');
    return { color: /^#[0-9a-f]{6}$/i.test(p.color) ? p.color : defaults.color, width: [2,3,5,8].includes(p.width) ? p.width : 3 };
  } catch { return { color: defaults.color, width: 3 }; }
}
function shortcut(e: KeyboardEvent): Action | undefined {
  if ((e.target as HTMLElement).closest('input,textarea,select,[contenteditable="true"]')) return;
  const key = e.key.toLowerCase();
  if (e.ctrlKey || e.metaKey) {
    if (key === 'z') return { action: e.shiftKey ? 'redo' : 'undo' };
    if (key === 'y') return { action: 'redo' };
    if (key === 'c') return { action: 'copy' };
    if (key === 's') return { action: 'save' };
    return;
  }
  if (key === 'escape') return { action: 'escape' };
  if (key === 'delete') return { action: e.shiftKey ? 'clear' : 'delete' };
  const tool = tools.find(t => t[3].toLowerCase() === key);
  if (tool && !e.altKey) return { action: 'tool', value: tool[0] };
}
function useContext() {
  const [context, setContext] = useState<Context>();
  useEffect(() => {
    let alive = true, revision = 0; let remove: UnlistenFn | undefined;
    void (async () => {
      const unlisten = await listen<Context|null>('screen-editor-reset', e => { revision++; if (alive) setContext(e.payload??undefined); });
      if (!alive) { unlisten(); return; } remove = unlisten;
      const requested = revision;
      const current = await invoke<Context | null>('screen_editor_context');
      if (alive && revision === requested) setContext(current??undefined);
    })().catch(() => {});
    return () => { alive = false; remove?.(); };
  }, []);
  return context;
}
export function ScreenInk() { const context = useContext(); return context ? <Ink key={context.id} context={context} /> : null; }
function Ink({ context }: { context: Context }) {
  const canvas = useRef<HTMLCanvasElement>(null), background = useRef<HTMLImageElement | null>(null);
  const history = useRef<History>(emptyHistory());
  const previousRect=useRef(context.rect);
  const draft = useRef<Mark | null>(null), moving = useRef<{ start: Point; mark: Mark } | null>(null);
  const frame = useRef(0), selected = useRef<string | undefined>(undefined);
  const [version, setVersion] = useState(0), [options, setOptions] = useState(preferences);
  const [tool, setTool] = useState<Tool>('pointer'), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  const [text, setText] = useState<{ point: Point; value: string }>();
  const actions = useRef<(a: Action) => void>(() => {}), live = useRef(true), working = useRef(false);
  const shown = useRef(false);
  const draw = () => {
    const ctx = canvas.current?.getContext('2d'); if (!ctx) return;
    ctx.setTransform(context.scale,0,0,context.scale,0,0); ctx.clearRect(0,0,context.width,context.height);
    if (background.current) ctx.drawImage(background.current,0,0,context.width,context.height);
    for (const mark of history.current.present) if (mark.id !== moving.current?.mark.id) paint(ctx,mark);
    if (draft.current) paint(ctx,draft.current);
    if (selected.current && context.kind === 'image' && tool === 'pointer') {
      const mark = history.current.present.find(m => m.id === selected.current); if (!mark) return;
      const b = bounds(mark); ctx.save(); ctx.strokeStyle = '#9775fa'; ctx.lineWidth = 1; ctx.setLineDash([4,3]); ctx.strokeRect(b.x-4,b.y-4,b.width+8,b.height+8); ctx.restore();
    }
  };
  const schedule = () => { if (!frame.current) frame.current = requestAnimationFrame(() => { frame.current = 0; draw(); }); };
  const changed = () => { setVersion(v => v+1); schedule(); };
  const commitText = () => {
    if (text?.value.trim()) {
      const mark: Mark = { id: crypto.randomUUID(), tool: 'text', ...options, points: [text.point], text: text.value.slice(0,300) };
      history.current = commit(history.current,[...history.current.present,mark]); changed();
    }
    setText(undefined);
  };
  const finishBlur=async(mark:Mark)=>{
    const b=bounds(mark);if(b.width<2||b.height<2){draft.current=null;schedule();return;}
    working.current=true;setBusy(true);
    try{
      if(context.kind==='image'&&background.current)mark.bitmap=blurredTile(background.current,b.width,b.height,context.scale,b);
      else{
        const bytes=await invoke<number[]>('screen_editor_sample',{id:context.id,rect:b});
        const url=URL.createObjectURL(new Blob([new Uint8Array(bytes)],{type:'image/png'}));
        try{const image=new Image();image.src=url;await image.decode();mark.bitmap=blurredTile(image,b.width,b.height,context.scale);}finally{URL.revokeObjectURL(url);}
      }
      if(live.current)history.current=commit(history.current,[...history.current.present,mark]);
    }catch(e){if(live.current)setError(String(e));}
    finally{draft.current=null;working.current=false;if(live.current){setBusy(false);changed();}}
  };
  const exportImage = async (action: 'copy' | 'save') => {
    if (context.kind !== 'image' || working.current || !background.current) return;
    working.current = true; setBusy(true); setError(''); selected.current = undefined; draw();
    try {
      const result = document.createElement('canvas'); result.width = Math.round(context.width*context.scale); result.height = Math.round(context.height*context.scale);
      const ctx = result.getContext('2d')!; ctx.setTransform(context.scale,0,0,context.scale,0,0);
      ctx.drawImage(background.current,0,0,context.width,context.height);
      for (const mark of history.current.present) paint(ctx,mark);
      const blob = await new Promise<Blob>((resolve,reject) => result.toBlob(b => b ? resolve(b) : reject(new Error('No se pudo preparar la imagen')), 'image/png'));
      await invoke('screen_image_export', { id: context.id, action, bytes: Array.from(new Uint8Array(await blob.arrayBuffer())) });
    } catch (e) { if (live.current) setError(String(e)); }
    finally { working.current = false; if (live.current) setBusy(false); }
  };
  actions.current = a => {
    if (working.current && !['tool','color','width','close','escape'].includes(a.action)) return;
    setError('');
    if (a.action === 'tool') { selected.current = undefined; setTool(a.value as Tool); }
    else if (a.action === 'color' && /^#[0-9a-f]{6}$/i.test(a.value ?? '')) setOptions(o => ({ ...o, color: a.value! }));
    else if (a.action === 'width') setOptions(o => ({ ...o, width: Math.min(8,Math.max(2,Number(a.value)||3)) }));
    else if (a.action === 'undo') { if (draft.current) draft.current = null; else history.current = undo(history.current); selected.current = undefined; changed(); }
    else if (a.action === 'redo') { history.current = redo(history.current); selected.current = undefined; changed(); }
    else if (a.action === 'clear' && history.current.present.length) { history.current = commit(history.current,[]); selected.current = undefined; changed(); }
    else if (a.action === 'delete' && selected.current) { history.current = commit(history.current,history.current.present.filter(m => m.id !== selected.current)); selected.current = undefined; changed(); }
    else if (a.action === 'copy' || a.action === 'save') void exportImage(a.action);
    else if (a.action === 'print' && context.kind === 'image') { selected.current = undefined; draw(); void invoke('screen_editor_print',{ id:context.id }).catch(e => setError(String(e))); }
    else if (a.action === 'reselect' && context.kind === 'image') void invoke('screen_select_image').catch(e => setError(String(e)));
    else if (a.action === 'close' || a.action === 'escape') {
      if (context.kind === 'image') void invoke('screen_cancel_selection').catch(e => setError(String(e)));
      else void invoke('screen_escape').catch(e => setError(String(e)));
    }
  };
  useEffect(() => {
    live.current = true; let unlisten: UnlistenFn | undefined;
    void (async () => {
      const remove = await listen<{ id:string; action:Action }>('screen-editor-action', e => { if (e.payload.id === context.id) actions.current(e.payload.action); });
      if (!live.current) { remove(); return; } unlisten = remove;
      if(context.kind==='video'){draw();await showWhenReady('screen_editor_ready',{id:context.id},()=>live.current);}
    })().catch(e => { if (live.current) setError(String(e)); });
    const key = (e:KeyboardEvent) => {
      const action = shortcut(e); if (!action) return;
      if (context.kind === 'video' && ['copy','save'].includes(action.action)) return;
      e.preventDefault();
      if (action.action === 'tool') void invoke('screen_editor_action',{ id:context.id,action }).catch(e => setError(String(e)));
      else actions.current(action);
    };
    window.addEventListener('keydown',key);
    return () => { live.current = false; unlisten?.(); cancelAnimationFrame(frame.current); window.removeEventListener('keydown',key); };
  }, [context.id]);
  useLayoutEffect(()=>{
    let alive=true,url='';
    cancelAnimationFrame(frame.current);frame.current=0;
    const previous=previousRect.current,next=context.rect;
    if(previous&&next&&(previous.x!==next.x||previous.y!==next.y)){
      const shift=(marks:Mark[])=>marks.map(mark=>({...mark,points:mark.points.map(p=>({x:p.x+previous.x-next.x,y:p.y+previous.y-next.y}))}));
      history.current={past:history.current.past.map(shift),present:shift(history.current.present),future:history.current.future.map(shift)};
    }
    previousRect.current=next;draft.current=null;moving.current=null;setText(undefined);
    // Changing canvas dimensions clears its bitmap. Repaint before the browser
    // presents that frame, keeping the previous image until the new crop decodes.
    draw();
    if(context.kind==='image')void(async()=>{
      working.current=true;setBusy(true);setError('');
      const bytes=await invoke<ArrayBuffer|number[]>('screen_editor_image',{id:context.id});
      if(!alive)return;
      url=URL.createObjectURL(new Blob([bytes instanceof ArrayBuffer?bytes:new Uint8Array(bytes)],{type:'image/png'}));
      const image=new Image();image.src=url;await image.decode();
      if(!alive)return;background.current=image;draw();
      // Re-cropping an already visible editor must not steal focus from its controls.
      if(!shown.current){await showWhenReady('screen_editor_ready',{id:context.id},()=>alive);if(alive)shown.current=true;}
    })().catch(e=>{if(alive)setError(String(e));}).finally(()=>{if(alive){working.current=false;setBusy(false);}});
    return()=>{alive=false;if(url)URL.revokeObjectURL(url);};
  },[context.id,context.rect?.x,context.rect?.y,context.width,context.height]);
  useEffect(() => {
    draw();
    try { localStorage.setItem('whispera.ink.v1',JSON.stringify(options)); } catch { /* Storage is optional. */ }
    void invoke('screen_editor_feedback',{ id:context.id,feedback:{ tool,...options,canUndo:!!history.current.past.length,canRedo:!!history.current.future.length,count:history.current.present.length,busy,error } }).catch(() => {});
  }, [version,tool,options,busy,error,context.id]);
  const point = (e: React.PointerEvent) => ({ x:Math.max(0,Math.min(context.width,e.clientX)),y:Math.max(0,Math.min(context.height,e.clientY)) });
  return <div className="screen-ink" data-tool={tool} data-session={context.id} data-kind={context.kind}>
    {context.kind==='image'&&!context.rect&&<CaptureFrame className="screen-image-frame" width={context.width} height={context.height} color={context.frame_color}/>}
    <canvas ref={canvas} style={context.kind === 'image' ? {borderRadius:Math.min(14,context.width/2,context.height/2)} : undefined} aria-label={context.kind === 'image' ? 'Editar captura' : 'Dibujar sobre video'} width={Math.round(context.width*context.scale)} height={Math.round(context.height*context.scale)}
      onPointerDown={e => {
        if (e.button !== 0 || busy) return; e.preventDefault(); commitText(); const p = point(e); e.currentTarget.setPointerCapture(e.pointerId);
        if (tool === 'text') { setText({point:p,value:''}); return; }
        if (tool === 'pointer') {
          const mark = hitTest(history.current.present,p); selected.current = mark?.id;
          moving.current = mark ? { start:p,mark } : null; draft.current = mark ?? null; schedule(); return;
        }
        selected.current = undefined;
        if (history.current.present.length >= 500) { setError('Borrá algunas marcas para seguir dibujando.'); return; }
        draft.current = { id:crypto.randomUUID(),tool,...options,points:[p] }; schedule();
      }} onPointerMove={e => {
        const mark = draft.current; if (!mark) return; const p = point(e);
        if (moving.current) { const start = moving.current.start; draft.current = {...mark,points:moving.current.mark.points.map(v => ({x:v.x+p.x-start.x,y:v.y+p.y-start.y}))}; }
        else if (mark.tool === 'pen' || mark.tool === 'highlight') { if (mark.points.length < 5000) mark.points.push(p); }
        else mark.points = [mark.points[0],constrained(mark.points[0],p,mark.tool,e.shiftKey)];
        schedule();
      }} onPointerUp={() => {
        if (!draft.current) return; const mark = draft.current;
        if (mark.tool==='blur'&&!moving.current) {void finishBlur(mark);return;}
        if (moving.current) {
          if (JSON.stringify(mark.points) !== JSON.stringify(moving.current.mark.points)) history.current = commit(history.current,history.current.present.map(m => m.id === mark.id ? mark : m));
        } else history.current = commit(history.current,[...history.current.present,mark]);
        draft.current = null; moving.current = null; changed();
      }} onPointerCancel={() => { draft.current = null; moving.current = null; schedule(); }} />
    {text && <input autoFocus className="screen-text-entry" aria-label="Texto de la marca" maxLength={300} style={{left:Math.min(text.point.x,Math.max(0,context.width-180)),top:Math.min(text.point.y,Math.max(0,context.height-40)),color:options.color,fontSize:14+options.width*3}} value={text.value} onChange={e => setText({...text,value:e.target.value})} onBlur={commitText} onKeyDown={e => { if(e.key==='Enter'){e.preventDefault();commitText();} if(e.key==='Escape'){e.stopPropagation();setText(undefined);} }} />}
  </div>;
}
export function ScreenTools() { const context = useContext(); return context ? <Toolbar key={context.id} context={context}/> : null; }
export function ScreenHud() { const context = useContext(); return context ? context.kind==='video' ? <VideoHud key={context.id} context={context}/> : <ImageActions key={context.id} context={context}/> : null; }
type VideoStatus = {phase:string;seconds:number;error:string};
function VideoHud({context}:{context:Context}) {
  const [status,setStatus]=useState<VideoStatus>({phase:'starting',seconds:0,error:''});
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  useEffect(()=>{
    let alive=true;let timer:ReturnType<typeof setTimeout>;
    const poll=async()=>{try{const value=await invoke<VideoStatus>('screen_status');if(alive)setStatus(old=>old.phase===value.phase&&Math.floor(old.seconds)===Math.floor(value.seconds)&&old.error===value.error?old:value);}catch(e){if(alive)setError(String(e));}finally{if(alive)timer=setTimeout(poll,400);}};
    void (async()=>{await invoke('screen_overlay_layout',{id:context.id,compact:true});if(alive){await showWhenReady('screen_editor_ready',{id:context.id},()=>alive);void poll();}})().catch(e=>{if(alive)setError(String(e));});
    return()=>{alive=false;clearTimeout(timer);};
  },[context.id]);
  const run=async(command:string)=>{setBusy(true);setError('');try{await invoke(command,command==='screen_video_snapshot'?{id:context.id}:undefined);}catch(e){setError(String(e));}finally{setBusy(false);}};
  const paused=status.phase==='paused';
  const label=paused?'En pausa':status.phase==='pausing'?'Pausando…':status.phase==='resuming'?'Reanudando…':status.phase==='saving'?'Preparando…':'Grabando';
  const time=`${Math.floor(status.seconds/60).toString().padStart(2,'0')}:${Math.floor(status.seconds%60).toString().padStart(2,'0')}`;
  return <div className="capture-hud capture-line-hud" data-compact="true" data-paused={paused} aria-label="Controles de video">
    <div className="capture-hud-controls"><span className="capture-hud-time" title={label}><i/><output aria-label="Tiempo grabado">{time}</output></span><button disabled={busy||!['recording','paused'].includes(status.phase)} aria-label="Capturar imagen del video" title="Capturar y copiar el área grabada" onClick={()=>void run('screen_video_snapshot')}><Camera size={16}/></button><button disabled={busy||!['recording','paused'].includes(status.phase)} aria-label={paused?'Reanudar video':'Pausar video'} title={paused?'Reanudar':'Pausar'} onClick={()=>void run('screen_pause')}><span className="t-icon-swap" data-state={paused?'b':'a'}><span className="t-icon" data-icon="a"><Pause size={16}/></span><span className="t-icon" data-icon="b"><Play size={16}/></span></span></button><button className="capture-stop" disabled={busy||!['recording','paused'].includes(status.phase)} aria-label="Detener video" title="Detener y guardar video" onClick={()=>void run('screen_stop')}><Square size={12} fill="currentColor"/></button><button disabled={busy||['saving','cancelling'].includes(status.phase)} aria-label="Cancelar video" title="Cancelar y descartar el video" onClick={()=>void run('screen_cancel')}><X size={16}/></button></div>
    {(error||status.error)&&<div role="alert" className="capture-hud-error">{error||status.error}</div>}
  </div>;
}
function useEditorControls(context:Context) {
  const [feedback,setFeedback] = useState<Feedback>({...defaults,...preferences()});
  const [error,setError] = useState('');
  const send = (action:Action) => {setError('');return invoke('screen_editor_action',{ id:context.id,action }).catch(e => setError(String(e)));};
  useEffect(() => {
    let alive = true, revision = 0; let remove:UnlistenFn|undefined;
    void (async () => {
      const off = await listen<{id:string;feedback:Partial<Feedback>}>('screen-editor-feedback',e => { if(alive && e.payload.id===context.id) {revision++;setFeedback(f => ({...f,...e.payload.feedback}));} });
      if(!alive){off();return;} remove=off;
      const requested=revision;
      const current = await invoke<Partial<Feedback>>('screen_editor_feedback_get'); if(alive&&revision===requested) setFeedback(f=>({...f,...current}));
      await showWhenReady('screen_editor_ready',{id:context.id},()=>alive);
    })().catch(e=>{if(alive)setError(String(e));});
    const key=(e:KeyboardEvent)=>{const a=shortcut(e);if(!a)return;if(context.kind==='video'&&['copy','save'].includes(a.action))return;e.preventDefault();if(a.action==='escape')void invoke('screen_escape').catch(e=>setError(String(e)));else void send(a);};
    window.addEventListener('keydown',key);
    return()=>{alive=false;remove?.();window.removeEventListener('keydown',key);};
  },[context.id]);
  return {feedback,send,setError,notice:(error||feedback.error)&&<div className="capture-error" role="alert">{error||feedback.error}<button aria-label="Cerrar aviso" onClick={()=>{setError('');setFeedback(f=>({...f,error:''}));}}>×</button></div>};
}
function ImageActions({context}:{context:Context}) {
  const {feedback,send,notice}=useEditorControls(context);
  return <div className="capture-image-actions" aria-label="Acciones de captura">
    <button className="capture-copy" disabled={feedback.busy} title="Copiar (Ctrl+C)" onClick={()=>void send({action:'copy'})}><Copy size={17}/>Copiar</button>
    <button disabled={feedback.busy} title="Guardar PNG (Ctrl+S)" aria-label="Guardar imagen" onClick={()=>void send({action:'save'})}><Save size={18}/></button>
    <button disabled={feedback.busy} title="Imprimir" aria-label="Imprimir imagen" onClick={()=>void send({action:'print'})}><Printer size={18}/></button>
    <button disabled={feedback.busy} title="Volver a seleccionar" aria-label="Volver a seleccionar" onClick={()=>void send({action:'reselect'})}><RotateCcw size={18}/></button>
    {notice}
  </div>;
}
function Toolbar({ context }: { context:Context }) {
  const {feedback,send,setError,notice}=useEditorControls(context);
  const [compact,setCompact] = useState(false);
  const [shape,setShape]=useState<Shape>('rectangle');
  const [panel,setPanel]=useState<'shapes'|'color'|'width'|'more'>();
  const [closing,setClosing]=useState(false);
  const [layout,setLayout]=useState({railX:4,railY:4,menuX:58,menuY:4});
  const timer=useRef<ReturnType<typeof setTimeout>>(undefined), epoch=useRef(0);
  const panelElement=useRef<HTMLDivElement>(null), panelAnchor=useRef(0);
  useEffect(()=>{
    let alive=true,remove:UnlistenFn|undefined;
    void listen<string>('screen-editor-drag',e=>{
      if(!alive||e.payload!==context.id)return;
      clearTimeout(timer.current);++epoch.current;
      setPanel(undefined);setClosing(false);setLayout({railX:4,railY:4,menuX:58,menuY:4});
    }).then(off=>{if(alive)remove=off;else off();});
    return()=>{alive=false;remove?.();};
  },[context.id]);
  const closePanel=()=>{
    const request=++epoch.current;setClosing(true);clearTimeout(timer.current);
    const duration=matchMedia('(prefers-reduced-motion: reduce)').matches?0:parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--dropdown-close-dur'))||150;
    timer.current=setTimeout(()=>{setPanel(undefined);setClosing(false);void invoke<typeof layout>('screen_tools_panel',{id:context.id,open:false,compact,anchor:0,panelHeight:100}).then(next=>{if(request===epoch.current)setLayout(next);}).catch(e=>setError(String(e)));},duration);
  };
  const openPanel=async(name:NonNullable<typeof panel>,button:HTMLButtonElement)=>{
    if(panel===name&&!closing){closePanel();return;}
    const request=++epoch.current;clearTimeout(timer.current);setClosing(false);
    const anchor=button.getBoundingClientRect().top-layout.railY+16;
    panelAnchor.current=anchor;
    try{const next=await invoke<typeof layout>('screen_tools_panel',{id:context.id,open:true,compact,anchor,panelHeight:name==='color'?380:name==='shapes'?208:name==='more'?240:210});if(request===epoch.current){setLayout(next);setPanel(name);}}catch(e){setError(String(e));}
  };
  useEffect(()=>{if(!panel)return;const blur=()=>closePanel();const outside=(e:PointerEvent)=>{if(!(e.target as HTMLElement).closest('.capture-line,.capture-line-panel'))closePanel();};const key=(e:KeyboardEvent)=>{if(e.key==='Escape'){e.preventDefault();e.stopImmediatePropagation();closePanel();}};window.addEventListener('blur',blur);window.addEventListener('pointerdown',outside);window.addEventListener('keydown',key,true);return()=>{window.removeEventListener('blur',blur);window.removeEventListener('pointerdown',outside);window.removeEventListener('keydown',key,true);};},[panel,compact]);
  useEffect(()=>()=>{clearTimeout(timer.current);++epoch.current;},[]);
  useEffect(()=>{
    const element=panelElement.current;if(!element||closing)return;
    const request=epoch.current;let height=0;
    const observer=new ResizeObserver(()=>{const next=element.offsetHeight+8;if(next===height)return;height=next;void invoke<typeof layout>('screen_tools_panel',{id:context.id,open:true,compact,anchor:panelAnchor.current,panelHeight:height}).then(value=>{if(epoch.current===request)setLayout(value);}).catch(e=>setError(String(e)));});
    observer.observe(element);return()=>observer.disconnect();
  },[panel,closing,compact,context.id]);
  const choose=async(value:Tool)=>{closePanel();await send({action:'tool',value});};
  const ShapeIcon=shapes.find(s=>s[0]===(isShape(feedback.tool)?feedback.tool:shape))![1];
  const toggleCompact=async()=>{
    try { clearTimeout(timer.current);++epoch.current;setPanel(undefined);setClosing(false);if(!compact&&context.kind==='video')await send({action:'tool',value:'pointer'});await invoke('screen_overlay_layout',{id:context.id,compact:!compact});setLayout({railX:4,railY:4,menuX:58,menuY:4});setCompact(!compact); }catch(e){setError(String(e));}
  };
  return <><div className="capture-toolbar capture-line" style={{left:layout.railX,top:layout.railY}} data-session={context.id} data-compact={compact} aria-label="Herramientas de captura">
    {compact ? <button className="capture-pointer" aria-label="Usar mouse normal" title="Volver al mouse (V)" aria-pressed={feedback.tool==='pointer'} onClick={()=>void send({action:'tool',value:'pointer'})}><MousePointer2 size={18}/></button> : <>
      <div className="capture-tool-list" role="toolbar" aria-label="Dibujo" aria-orientation="vertical">{tools.map(([name,Icon,label,key])=>name==='rectangle'?<button key={name} title="Formas" aria-label="Formas" aria-expanded={panel==='shapes'&&!closing} aria-pressed={isShape(feedback.tool)} disabled={feedback.busy} onClick={e=>void openPanel('shapes',e.currentTarget)}><ShapeIcon size={17}/></button>:<button key={name} title={`${label} (${key})`} aria-label={label} aria-pressed={feedback.tool===name} disabled={feedback.busy} onClick={()=>void send({action:'tool',value:name})}><Icon size={17} strokeWidth={1.7}/></button>)}</div>
      <div className="capture-line-style"><button aria-label="Elegir color" title="Color del trazo" aria-expanded={panel==='color'&&!closing} onClick={e=>void openPanel('color',e.currentTarget)}><i className="capture-line-swatch" style={{background:feedback.color}}/></button><button title="Grosor" aria-label="Grosor" aria-expanded={panel==='width'&&!closing} onClick={e=>void openPanel('width',e.currentTarget)}><SlidersHorizontal size={17}/></button><button title="Más herramientas" aria-label="Más herramientas" aria-expanded={panel==='more'&&!closing} onClick={e=>void openPanel('more',e.currentTarget)}><MoreHorizontal size={17}/></button></div>
    </>}
    <div className="capture-tool-header"><button aria-label={compact?'Expandir herramientas':'Contraer herramientas'} title={compact?'Expandir herramientas':'Contraer herramientas'} onClick={()=>void toggleCompact()}>{compact?<ChevronRight size={17}/>:<ChevronLeft size={17}/>}</button></div>
    {notice}
  </div>{panel&&<div ref={panelElement} className={`capture-line-panel t-dropdown ${closing?'is-closing':'is-open'}`} style={{left:layout.menuX,top:layout.menuY}} data-origin="top-center" role="dialog" aria-label={panel==='shapes'?'Formas':panel==='color'?'Color del trazo':panel==='width'?'Grosor':'Más herramientas'}>
    {panel==='color'?<div className="palette-dialog"><PaletteContents title="Color del trazo" value={{base:'black',hex:feedback.color}} onChange={value=>void send({action:'color',value:selectionHex(value)})} close={closePanel}/></div>:<><div className="capture-panel-heading">{panel==='shapes'?'Formas':panel==='width'?'Grosor':'Más herramientas'}<button title="Cerrar panel" aria-label="Cerrar panel" onClick={closePanel}><X size={15}/></button></div>
    {panel==='shapes'?<div className="capture-shapes">{shapes.map(([name,Icon,label])=><button key={name} aria-label={label} aria-pressed={feedback.tool===name} onClick={()=>{setShape(name);void choose(name);}}><Icon size={22}/><span>{label==='Círculo / elipse'?'Elipse':label}</span></button>)}</div>:panel==='width'?<div className="capture-stroke-options">{([[2,'Fino'],[3,'Normal'],[5,'Medio'],[8,'Grueso']] as const).map(([width,label])=><button key={width} aria-label={`Grosor ${label.toLowerCase()}`} aria-pressed={feedback.width===width} onClick={()=>void send({action:'width',value:String(width)})}><i style={{height:width}}/>{width} px</button>)}</div>:<div className="capture-menu-actions">{([['undo',Undo2,'Deshacer',!feedback.canUndo],['redo',Redo2,'Rehacer',!feedback.canRedo],['delete',X,'Eliminar selección',!feedback.count],['clear',Trash2,'Borrar todas las marcas',!feedback.count]] as const).map(([action,Icon,label,disabled])=><button key={action} disabled={disabled||feedback.busy} onClick={()=>void send({action})}><Icon size={16}/>{label}</button>)}</div>}</>}
  </div>}</>;
}
