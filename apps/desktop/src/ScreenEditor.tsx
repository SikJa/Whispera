import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MotionConfig } from 'motion/react';
import ControlledFolder from './ControlledFolder';
import { contrastInk } from './palette';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ArrowUpRight, ChevronDown, ChevronUp, GripHorizontal, Maximize2, Pause, Play, Copy, Highlighter, Minus, MousePointer2, Pencil, Printer, Redo2, RotateCcw, Save, Square, SquareDashed, Trash2, Type, Undo2, X } from 'lucide-react';
import { blurredTile, bounds, commit, constrained, emptyHistory, hitTest, paint, redo, undo, type History, type Mark, type Point, type Tool } from './screen-annotations';
import './screen-recorder.css';
import { showWhenReady } from './screen-ready';

type Context = { id: string; kind: 'image' | 'video'; width: number; height: number; scale: number; hud_scale?:number; frame_color?:string };
type Action = { action: string; value?: string };
type Feedback = { tool: Tool; color: string; width: number; canUndo: boolean; canRedo: boolean; count: number; busy: boolean; error: string };
const defaults: Feedback = { tool: 'pointer', color: '#ff4545', width: 3, canUndo: false, canRedo: false, count: 0, busy: false, error: '' };
const tools = [
  ['pointer', MousePointer2, 'Puntero / seleccionar', 'V'], ['pen', Pencil, 'Lápiz', 'P'], ['line', Minus, 'Línea', 'L'],
  ['arrow', ArrowUpRight, 'Flecha', 'A'], ['rectangle', Square, 'Recuadro', 'R'], ['highlight', Highlighter, 'Resaltador', 'H'],
  ['text', Type, 'Texto', 'T'], ['blur', SquareDashed, 'Bloque difuminado', 'B'],
] as const;
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
    let alive = true; let remove: UnlistenFn | undefined;
    void (async () => {
      const unlisten = await listen<Context|null>('screen-editor-reset', e => { if (alive) setContext(e.payload??undefined); });
      if (!alive) { unlisten(); return; } remove = unlisten;
      const current = await invoke<Context | null>('screen_editor_context');
      if (alive && current) setContext(current);
    })().catch(() => {});
    return () => { alive = false; remove?.(); };
  }, []);
  return context;
}
export function ScreenInk() { const context = useContext(); return context ? <Ink key={context.id} context={context} /> : null; }
function Ink({ context }: { context: Context }) {
  const canvas = useRef<HTMLCanvasElement>(null), background = useRef<HTMLImageElement | null>(null);
  const history = useRef<History>(emptyHistory());
  const draft = useRef<Mark | null>(null), moving = useRef<{ start: Point; mark: Mark } | null>(null);
  const frame = useRef(0), selected = useRef<string | undefined>(undefined);
  const [version, setVersion] = useState(0), [options, setOptions] = useState(preferences);
  const [tool, setTool] = useState<Tool>('pointer'), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  const [text, setText] = useState<{ point: Point; value: string }>();
  const actions = useRef<(a: Action) => void>(() => {}), live = useRef(true), working = useRef(false);
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
    if (working.current) return;
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
    live.current = true; let unlisten: UnlistenFn | undefined; let imageUrl = '';
    void (async () => {
      const remove = await listen<{ id:string; action:Action }>('screen-editor-action', e => { if (e.payload.id === context.id) actions.current(e.payload.action); });
      if (!live.current) { remove(); return; } unlisten = remove;
      if (context.kind === 'image') {
        const bytes = await invoke<number[]>('screen_editor_image',{ id:context.id });
        if (!live.current) return;
        imageUrl = URL.createObjectURL(new Blob([new Uint8Array(bytes)],{ type:'image/png' }));
        const image = new Image(); image.src = imageUrl; await image.decode();
        if (!live.current) return; background.current = image;
      }
      draw();await showWhenReady('screen_editor_ready',{id:context.id},()=>live.current);
    })().catch(e => { if (live.current) setError(String(e)); });
    const key = (e:KeyboardEvent) => {
      const action = shortcut(e); if (!action) return;
      if (context.kind === 'video' && ['copy','save'].includes(action.action)) return;
      e.preventDefault();
      if (action.action === 'tool') void invoke('screen_editor_action',{ id:context.id,action }).catch(e => setError(String(e)));
      else actions.current(action);
    };
    window.addEventListener('keydown',key);
    return () => { live.current = false; unlisten?.(); cancelAnimationFrame(frame.current); window.removeEventListener('keydown',key); if(imageUrl) URL.revokeObjectURL(imageUrl); };
  }, [context.id]);
  useEffect(() => {
    draw();
    try { localStorage.setItem('whispera.ink.v1',JSON.stringify(options)); } catch { /* Storage is optional. */ }
    void invoke('screen_editor_feedback',{ id:context.id,feedback:{ tool,...options,canUndo:!!history.current.past.length,canRedo:!!history.current.future.length,count:history.current.present.length,busy,error } }).catch(() => {});
  }, [version,tool,options,busy,error,context.id]);
  const point = (e: React.PointerEvent) => ({ x:Math.max(0,Math.min(context.width,e.clientX)),y:Math.max(0,Math.min(context.height,e.clientY)) });
  return <div className="screen-ink" data-tool={tool} data-session={context.id} data-kind={context.kind} style={{'--capture-frame-color':context.frame_color??'#ffffff'} as React.CSSProperties}>
    <canvas ref={canvas} aria-label={context.kind === 'image' ? 'Editar captura' : 'Dibujar sobre video'} width={Math.round(context.width*context.scale)} height={Math.round(context.height*context.scale)}
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
export function ScreenHud() { const context = useContext(); return context?.kind==='video' ? <VideoHud key={context.id} context={context}/> : null; }
type VideoStatus = {phase:string;seconds:number;error:string};
function VideoHud({context}:{context:Context}) {
  const [status,setStatus]=useState<VideoStatus>({phase:'starting',seconds:0,error:''});
  const [appearance,setAppearance]=useState({color:'#9024DC',pattern:'wave' as 'wave'|'stairs'});
  const [compact,setCompact]=useState(()=>{try{return localStorage.getItem('whispera.video.compact')==='true';}catch{return false;}});
  const [step,setStep]=useState(0);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  useEffect(()=>{
    let alive=true;let timer:ReturnType<typeof setTimeout>;
    const poll=async()=>{try{const value=await invoke<VideoStatus>('screen_status');if(alive)setStatus(old=>old.phase===value.phase&&Math.floor(old.seconds)===Math.floor(value.seconds)&&old.error===value.error?old:value);}catch(e){if(alive)setError(String(e));}finally{if(alive)timer=setTimeout(poll,400);}};
    void (async()=>{const data=await invoke<typeof appearance>('screen_appearance');if(!alive)return;setAppearance(data);await invoke('screen_overlay_layout',{id:context.id,compact});if(alive){await showWhenReady('screen_editor_ready',{id:context.id},()=>alive);void poll();}})().catch(e=>{if(alive)setError(String(e));});
    return()=>{alive=false;clearTimeout(timer);};
  },[context.id]);
  useEffect(()=>{if(compact||status.phase!=='recording')return;const timer=setInterval(()=>setStep(v=>v+1),650);return()=>clearInterval(timer);},[compact,status.phase]);
  const run=async(command:string)=>{setBusy(true);setError('');try{await invoke(command);}catch(e){setError(String(e));}finally{setBusy(false);}};
  const toggle=async()=>{try{await invoke('screen_overlay_layout',{id:context.id,compact:!compact});setCompact(!compact);try{localStorage.setItem('whispera.video.compact',String(!compact));}catch{/* Optional preference. */}}catch(e){setError(String(e));}};
  const dragging=(e:React.PointerEvent)=>{if(e.button===0&&!(e.target as HTMLElement).closest('button'))void getCurrentWindow().startDragging().catch(e=>setError(String(e)));};
  const paused=status.phase==='paused';
  const label=paused?'En pausa':status.phase==='pausing'?'Pausando…':status.phase==='resuming'?'Reanudando…':status.phase==='saving'?'Preparando…':'Grabando';
  const time=`${Math.floor(status.seconds/60).toString().padStart(2,'0')}:${Math.floor(status.seconds%60).toString().padStart(2,'0')}`;
  const scale=context.hud_scale??.85;
  return <div className="capture-hud" data-compact={compact} data-paused={paused} onPointerDown={dragging} aria-label="Controles de video">
    {!compact&&<div className="capture-hud-folder" style={{top:180*scale,left:12,width:321,height:270,transform:`scale(${scale})`,transformOrigin:'top left'}}><MotionConfig reducedMotion="user"><ControlledFolder color="black" customColor={appearance.color} size="md" pattern={appearance.pattern} visualState={paused?'rest':'hover'} recordingStep={status.phase==='recording'?step:undefined}/></MotionConfig><div className="screen-folder-caption" style={{color:contrastInk(appearance.color)}}><span role="status"><i/>{label}</span><output aria-label="Tiempo grabado">{time}</output></div></div>}
    <div className="capture-hud-controls"><span className="capture-hud-grip" title="Arrastrar controles"><GripHorizontal size={15}/></span>{compact&&<span className="capture-hud-time" title={label}><i/>{time}</span>}<button disabled={busy||!['recording','paused'].includes(status.phase)} aria-label={paused?'Reanudar video':'Pausar video'} title={paused?'Reanudar':'Pausar'} onClick={()=>void run('screen_pause')}>{paused?<Play size={16}/>:<Pause size={16}/>}</button><button disabled={busy||['saving','cancelling'].includes(status.phase)} aria-label="Cancelar video" title="Cancelar y descartar el video" onClick={()=>void run('screen_cancel')}><X size={16}/></button><button aria-label={compact?'Mostrar carpeta':'Ocultar carpeta'} title={compact?'Mostrar carpeta':'Ocultar carpeta'} onClick={()=>void toggle()}>{compact?<Maximize2 size={15}/>:<Minus size={15}/>}</button></div>
    {(error||status.error)&&<div role="alert" className="capture-hud-error">{error||status.error}</div>}
  </div>;
}
function Toolbar({ context }: { context:Context }) {
  const [feedback,setFeedback] = useState<Feedback>({...defaults,...preferences()});
  const [error,setError] = useState('');
  const [compact,setCompact] = useState(false);
  const send = (action:Action) => invoke('screen_editor_action',{ id:context.id,action }).catch(e => setError(String(e)));
  useEffect(() => {
    let alive = true; let remove:UnlistenFn|undefined;
    void (async () => {
      const off = await listen<{id:string;feedback:Partial<Feedback>}>('screen-editor-feedback',e => { if(alive && e.payload.id===context.id) setFeedback(f => ({...f,...e.payload.feedback})); });
      if(!alive){off();return;} remove=off;
      const current = await invoke<Partial<Feedback>>('screen_editor_feedback_get'); if(alive) setFeedback(f=>({...f,...current}));
      await showWhenReady('screen_editor_ready',{id:context.id},()=>alive);
    })().catch(e=>{if(alive)setError(String(e));});
    const key=(e:KeyboardEvent)=>{const a=shortcut(e);if(!a)return;if(context.kind==='video'&&['copy','save'].includes(a.action))return;e.preventDefault();if(a.action==='escape')void invoke('screen_escape').catch(e=>setError(String(e)));else void send(a);};
    window.addEventListener('keydown',key);
    return()=>{alive=false;remove?.();window.removeEventListener('keydown',key);};
  },[context.id]);
  const toggleCompact=async()=>{
    try { if(!compact&&context.kind==='video')await send({action:'tool',value:'pointer'});await invoke('screen_overlay_layout',{id:context.id,compact:!compact});setCompact(!compact); }catch(e){setError(String(e));}
  };
  return <div className="capture-toolbar" data-session={context.id} data-compact={compact} aria-label="Herramientas de captura">
    <div className="capture-tool-header"><button className="capture-drag" aria-label="Mover herramientas" title="Arrastrar herramientas" onPointerDown={e=>{if(e.button===0)void getCurrentWindow().startDragging().catch(e=>setError(String(e)));}}><GripHorizontal size={17}/></button><span>Herramientas</span><button aria-label="Usar mouse normal" title="Volver al mouse (V)" aria-pressed={feedback.tool==='pointer'} onClick={()=>void send({action:'tool',value:'pointer'})}><MousePointer2 size={15}/></button><button aria-label={compact?'Expandir herramientas':'Contraer herramientas'} title={compact?'Expandir':'Contraer'} onClick={()=>void toggleCompact()}>{compact?<ChevronDown size={15}/>:<ChevronUp size={15}/>}</button></div>
    {!compact&&<>
    <div className="capture-tool-row" role="toolbar" aria-label="Dibujo">{tools.map(([name,Icon,label,key])=><button key={name} title={`${label} (${key})`} aria-label={label} aria-pressed={feedback.tool===name} disabled={feedback.busy} onClick={()=>void send({action:'tool',value:name})}><Icon size={18}/></button>)}
      <button title="Borrar todas las marcas (se puede deshacer)" aria-label="Borrar todas las marcas" disabled={!feedback.count||feedback.busy} onClick={()=>void send({action:'clear'})}><Trash2 size={17}/></button>
    </div>
    <div className="capture-tool-row capture-options">
      {['#ff4545','#ffca3a','#4cdb91','#5da9ff','#ffffff'].map(color=><button className="capture-swatch" key={color} style={{background:color}} aria-label={`Color ${color}`} aria-pressed={feedback.color===color} onClick={()=>void send({action:'color',value:color})}/>)}
      <input aria-label="Color personalizado" title="Color personalizado" type="color" value={feedback.color} onChange={e=>void send({action:'color',value:e.target.value})}/>
      <select aria-label="Grosor" value={feedback.width} onChange={e=>void send({action:'width',value:e.target.value})}>{[2,3,5,8].map(width=><option key={width} value={width}>{width} px</option>)}</select>
      <button title="Deshacer (Ctrl+Z)" aria-label="Deshacer" disabled={!feedback.canUndo||feedback.busy} onClick={()=>void send({action:'undo'})}><Undo2 size={18}/></button>
      <button title="Rehacer (Ctrl+Y / Ctrl+Shift+Z)" aria-label="Rehacer" disabled={!feedback.canRedo||feedback.busy} onClick={()=>void send({action:'redo'})}><Redo2 size={18}/></button>
    </div>
    {context.kind==='image'&&<div className="capture-tool-row capture-footer">
      <button className="capture-copy" disabled={feedback.busy} title="Copiar (Ctrl+C)" onClick={()=>void send({action:'copy'})}><Copy size={15}/>Copiar</button>
      <button disabled={feedback.busy} title="Guardar PNG (Ctrl+S)" aria-label="Guardar imagen" onClick={()=>void send({action:'save'})}><Save size={16}/></button>
      <button disabled={feedback.busy} title="Imprimir" aria-label="Imprimir imagen" onClick={()=>void send({action:'print'})}><Printer size={16}/></button>
      <button disabled={feedback.busy} title="Volver a seleccionar" aria-label="Volver a seleccionar" onClick={()=>void send({action:'reselect'})}><RotateCcw size={16}/></button>
    </div>}</>}
    {(error||feedback.error)&&<div className="capture-error" role="alert">{error||feedback.error}<button aria-label="Cerrar aviso" onClick={()=>{setError('');setFeedback(f=>({...f,error:''}));}}>×</button></div>}
  </div>;
}
