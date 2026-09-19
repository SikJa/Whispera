import {useEffect,useRef,useState} from 'react';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {invoke} from '@tauri-apps/api/core';
import {open} from '@tauri-apps/plugin-dialog';
import {Copy,Check,FolderOpen,Pencil,ArrowLeft} from 'lucide-react';
import ControlledFolder from './ControlledFolder';
import {native,snapshot} from './client';
import './file-import.css';
export default function FileImport(){
  const [phase,setPhase]=useState<'idle'|'processing'|'done'>('idle');
  const [seconds,setSeconds]=useState(0); const [text,setText]=useState(''); const [error,setError]=useState('');
  const [editing,setEditing]=useState(false); const [copied,setCopied]=useState(false); const [color,setColor]=useState('#9024DC');
  const busy=useRef(false);
  useEffect(()=>{void snapshot().then(s=>setColor(s.settings.color));},[]);
  useEffect(()=>{if(phase!=='processing')return;const timer=setInterval(()=>setSeconds(t=>t+1),1000);return()=>clearInterval(timer);},[phase]);
  async function transcribe(path:string){
    if(busy.current)return;busy.current=true;setError('');setPhase('processing');setSeconds(0);setCopied(false);
    try{setText(await invoke<string>('transcribe_file',{path}));setPhase('done');}catch(e){setError(String(e));setPhase('idle');}finally{busy.current=false;}
  }
  useEffect(()=>{
    if(!native)return; let disposed=false; let unlisten:(()=>void)|undefined;
    void getCurrentWindow().onDragDropEvent(e=>{if(e.payload.type==='drop'&&e.payload.paths.length===1)void transcribe(e.payload.paths[0]);}).then(fn=>{if(disposed)fn();else unlisten=fn;});
    return()=>{disposed=true;unlisten?.();};
  },[]);
  async function browse(){try{const path=await open({multiple:false,filters:[{name:'Audio',extensions:['wav','mp3','m4a','ogg','flac','webm','mp4']}]});if(typeof path==='string')void transcribe(path);}catch(e){setError(String(e));}}
  return <main className="file-import"><header><a href="?view=settings"><ArrowLeft size={16}/>Whispera</a><h1>Transcribir audio</h1></header>
    <div className="import-folder"><ControlledFolder color="black" customColor={color} size="sm" visualState={phase==='processing'?(seconds%2?'hover':'open'):'rest'}/></div>
    <div className="import-status" role="status">{phase==='processing'?'Transcribiendo':phase==='done'?'Transcripción lista':'Archivo de audio'}</div>
    {phase==='processing'?<output>{Math.floor(seconds/60).toString().padStart(2,'0')}:{(seconds%60).toString().padStart(2,'0')}</output>:phase==='idle'?<><button disabled={!native} className="import-primary" onClick={()=>void browse()}><FolderOpen size={17}/>Explorar</button><div className="import-drop">Arrastrá un audio acá</div></>:<><button className="import-primary" onClick={async()=>{try{await invoke('copy_text',{text});setCopied(true);}catch(e){setError(String(e));}}}>{copied?<Check size={17}/>:<Copy size={17}/>} {copied?'Copiado':'Copiar'}</button><button className="import-edit" onClick={()=>setEditing(v=>!v)}><Pencil size={14}/>{editing?'Cerrar editor':'Editar'}</button><button className="import-edit" onClick={()=>{setPhase('idle');setEditing(false);setText('');}}>Otro audio</button></>}
    {editing&&<textarea autoFocus aria-label="Editar transcripción importada" value={text} onChange={e=>{setText(e.target.value);setCopied(false);}}/>}
    {error&&<p role="alert">{error}</p>}
  </main>;
}
