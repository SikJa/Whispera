import {useEffect,useRef,useState} from 'react';
import {convertFileSrc,invoke} from '@tauri-apps/api/core';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {Scissors,X,Check,Copy} from 'lucide-react';
import './replay.css';
type Context={path:string;name:string;duration:number;hasAudio:boolean};
type Saved={path:string;transcript:boolean;warning:string};
const time=(n:number)=>`${Math.floor(n/60)}:${(n%60).toFixed(1).padStart(4,'0')}`;
export default function VideoTrim(){
  const [ctx,setCtx]=useState<Context>(),[start,setStart]=useState(0),[end,setEnd]=useState(0),[busy,setBusy]=useState(false),[error,setError]=useState(''),[saved,setSaved]=useState<Saved>();
  const video=useRef<HTMLVideoElement>(null),alive=useRef(true),gate=useRef(false);
  useEffect(()=>{alive.current=true;void invoke<Context>('video_trim_context').then(value=>{if(alive.current){setCtx(value);setEnd(value.duration);}}).catch(e=>{if(alive.current)setError(String(e));});return()=>{alive.current=false;};},[]);
  const close=()=>{if(!gate.current)void invoke('video_trim_close').catch(e=>setError(String(e)));};
  const save=async()=>{if(gate.current||!ctx)return;gate.current=true;setBusy(true);setError('');video.current?.pause();
    try{const result=await invoke<Saved>('video_trim_save',{start,end});if(alive.current)setSaved(result);}
    catch(e){if(alive.current)setError(String(e));}finally{gate.current=false;if(alive.current)setBusy(false);}
  };
  useEffect(()=>{const key=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.preventDefault();close();}const editingText=event.target instanceof HTMLInputElement&&event.target.type!=='range';if(event.ctrlKey&&event.key.toLowerCase()==='c'&&!editingText){event.preventDefault();void save();}};window.addEventListener('keydown',key);return()=>window.removeEventListener('keydown',key);});
  const changed=(a:number,b:number)=>{setStart(a);setEnd(b);setSaved(undefined);if(video.current)video.current.currentTime=a;};
  return <main className="trim-editor">
    <header className="trim-header" onMouseDown={e=>{if(e.button===0&&!(e.target as HTMLElement).closest('button'))void getCurrentWindow().startDragging();}}><span><Scissors size={17}/>Elegí el tramo</span><button aria-label="Cerrar editor" disabled={busy} onClick={close}><X size={18}/></button></header>
    {ctx&&<><video ref={video} src={convertFileSrc(ctx.path)} controls preload="metadata" onTimeUpdate={()=>{const player=video.current;if(player&&player.currentTime>=end&&!player.paused){player.pause();player.currentTime=start;}}}/>
      <div className="trim-selection"><div className="trim-times"><label>Desde <input aria-label="Inicio del tramo" type="number" min={0} max={end-0.15} step="0.1" value={Number(start.toFixed(2))} disabled={busy} onChange={e=>{const n=Number(e.target.value);if(Number.isFinite(n)&&n>=0&&n<=end-0.15)changed(n,end);}}/></label><strong>{time(end-start)}</strong><label>Hasta <input aria-label="Fin del tramo" type="number" min={start+0.15} max={ctx.duration} step="0.1" value={Number(end.toFixed(2))} disabled={busy} onChange={e=>{const n=Number(e.target.value);if(Number.isFinite(n)&&n>=start+0.15&&n<=ctx.duration)changed(start,n);}}/></label></div>
      <label className="trim-range">Inicio<input aria-label="Seleccionar inicio" type="range" min={0} max={ctx.duration} step="0.05" value={start} disabled={busy} onChange={e=>changed(Math.min(Number(e.target.value),end-0.15),end)}/></label>
      <label className="trim-range">Final<input aria-label="Seleccionar final" type="range" min={0} max={ctx.duration} step="0.05" value={end} disabled={busy} onChange={e=>changed(start,Math.max(Number(e.target.value),start+0.15))}/></label></div>
      <footer className="trim-footer"><small>{ctx.hasAudio?'Se guarda el video y la transcripción de este tramo.':'Video sin audio · no se genera transcripción.'}</small><button disabled={busy} onClick={()=>void save()}>{busy?<span className="trim-spinner"/>:saved?<Check size={16}/>:<Copy size={16}/>}<span>{busy?'Guardando…':saved?'Volver a copiar':'Guardar'}<small>También Ctrl+C · copia para pegar</small></span></button></footer></>}
    <p className="trim-message" role="status">{error||(saved?(saved.warning?`Video guardado. ${saved.warning}`:'Tramo guardado y copiado. Pegalo con Ctrl+V.'):(busy&&ctx?.hasAudio?'Preparando video y transcripción…':''))}</p>
  </main>;
}
