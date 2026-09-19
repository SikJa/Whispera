import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Mic, Square, Copy, Check, RotateCcw, FolderOpen, Pencil, X, Save, LoaderCircle } from 'lucide-react';
import { Dialog, Modal, ModalOverlay } from 'react-aria-components';
import { native, snapshot, type Settings } from './client';
import './recorder.css';

type State={phase:string;seconds:number;level:number;session:string;error:string;text:string;muted:boolean;progress:string};
type Recovery={id:string;createdAt:string;seconds:number;path:string};
const initial:State={phase:'idle',seconds:0,level:0,session:'',error:'',text:'',muted:false,progress:''};
const labels:Record<string,string>={idle:'Listo para grabar',recording:'Grabando',paused:'En pausa',ready:'Audio guardado',processing:'Transcribiendo',done:'Transcripción lista',error:'Audio pendiente'};
export default function Recorder(){
  const [state,setState]=useState(initial);const [settings,setSettings]=useState<Settings>();
  const [pending,setPending]=useState<Recovery[]>([]);const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  const [copied,setCopied]=useState(false);const [editing,setEditing]=useState(false);const [text,setText]=useState('');
  const recording=['recording','paused'].includes(state.phase);
  useEffect(()=>{let disposed=false;let timer:ReturnType<typeof setTimeout>;
    async function poll(){try{const [view,config,recoveries]=await Promise.all([invoke<State>('recording_state'),snapshot(),invoke<Recovery[]>('pending_recordings')]);if(!disposed){setState(view);setSettings(config.settings);setPending(recoveries);}}catch(e){if(!disposed)setError(String(e));}finally{if(!disposed)timer=setTimeout(poll,300);}}
    if(native)void poll();else{void snapshot().then(s=>setSettings(s.settings));}
    return()=>{disposed=true;clearTimeout(timer);};
  },[]);
  useEffect(()=>{setText(state.text);setCopied(false);},[state.text]);
  async function run(command:string,args?:Record<string,unknown>){setBusy(true);setError('');try{await invoke(command,args);}catch(e){setError(String(e));}finally{setBusy(false);}}
  const action=(action:string)=>run('recording_action',{action});
  const time=`${Math.floor(state.seconds/60).toString().padStart(2,'0')}:${Math.floor(state.seconds%60).toString().padStart(2,'0')}`;
  return <main className="live-recorder" data-phase={state.phase}>
    <header className="recording-header"><a href="?view=settings"><img src="/cristal/64x64.png" alt=""/>Whispera</a><span>Audios y transcripción</span></header>
    <div className="live-transport"><div><span className="live-state" role="status"><i data-recording={state.phase==='recording'}/>{labels[state.phase]??state.phase}</span><output aria-label="Tiempo grabado">{time}</output></div>
      <div className="live-actions">
        {!recording&&state.phase!=='processing'&&<button className="primary" disabled={busy||!native} onClick={()=>action('start')}><Mic size={18}/>{state.phase==='idle'?'Grabar':'Nueva grabación'}</button>}
        {recording&&<button className="primary" disabled={busy} onClick={()=>action('stop')}><Square size={17} fill="currentColor"/>Detener y transcribir</button>}
        {state.phase==='processing'&&<span className="live-processing"><LoaderCircle className="busy-spin" size={19}/>{state.progress}</span>}
      </div>
    </div>
    {(error||state.error)&&<div className="live-error" role="alert">{error||state.error}</div>}
    {recording&&<button className="live-secondary" disabled={busy} onClick={()=>action('save')}><Save size={15}/>Guardar sin transcribir</button>}
    {state.phase==='done'&&<div className="live-result"><button className="primary" onClick={async()=>{try{await invoke('copy_recording');setCopied(true);}catch(e){setError(String(e));}}}>{copied?<Check size={18}/>:<Copy size={18}/>} {copied?'Copiado':'Copiar'}</button><button className="live-secondary" onClick={()=>setEditing(true)}><Pencil size={14}/>Editar texto</button><button className="live-secondary" onClick={()=>run('reveal_recording',{id:state.session})}><FolderOpen size={14}/>Ver audio guardado</button></div>}
    {pending.length>0&&<section className="pending-audio"><h2>Audios pendientes</h2>{pending.map(item=><div key={item.id}><span>{new Date(item.createdAt).toLocaleString('es-AR')}<small>{Math.floor(item.seconds/60)} min {Math.floor(item.seconds%60)} s</small></span><button title="Reintentar transcripción" aria-label="Reintentar transcripción" disabled={busy||recording||state.phase==='processing'} onClick={()=>run('retry_recording',{id:item.id})}><RotateCcw size={17}/></button><button title="Abrir carpeta del audio" aria-label="Abrir carpeta del audio" disabled={busy||recording||state.phase==='processing'} onClick={()=>run('reveal_recording',{id:item.id})}><FolderOpen size={17}/></button></div>)}</section>}
    <ModalOverlay className="modal-backdrop desktop-app-modal" isOpen={editing} onOpenChange={setEditing} isDismissable><Modal className="transcript-modal"><Dialog aria-label="Editar transcripción"><div className="modal-heading"><h2>Transcripción</h2><button aria-label="Cerrar editor" onClick={()=>setEditing(false)}><X size={18}/></button></div><textarea aria-label="Texto transcrito" value={text} onChange={e=>setText(e.target.value)}/><button className="primary" onClick={()=>run('copy_text',{text})}><Copy size={17}/>Copiar texto editado</button></Dialog></Modal></ModalOverlay>
  </main>;
}
