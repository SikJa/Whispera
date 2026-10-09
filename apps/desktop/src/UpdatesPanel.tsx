import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen,type UnlistenFn} from '@tauri-apps/api/event';
import {Download,RefreshCw,ShieldCheck,LoaderCircle,Check,ExternalLink} from 'lucide-react';
import {native} from './client';
import './updates.css';

export type UpdateStatus={currentVersion:string;version:string|null;notes:string;phase:string;downloaded:number;total:number|null;error:string;upstream?:{version:string|null;notes:string;url:string;phase:string;error:string}};
const initial:UpdateStatus={currentVersion:'',version:null,notes:'',phase:'idle',downloaded:0,total:null,error:''};
export function briefUpdateNotes(notes:string){
  const text=notes.replace(/^#{1,6}\s+.*$/gm,'').replace(/^\s*[-*]\s+/gm,'').replace(/\s+/g,' ').trim();
  if(text.length<=280)return text;
  const cut=text.slice(0,277);return cut.slice(0,cut.lastIndexOf(' ')>200?cut.lastIndexOf(' '):cut.length)+'…';
}
export function useUpdates(){
  const [status,setStatus]=useState(initial),[error,setError]=useState('');
  useEffect(()=>{
    if(!native)return;
    let alive=true,revision=0,remove:UnlistenFn|undefined;
    void(async()=>{
      const off=await listen<UpdateStatus>('updater-status',e=>{revision++;if(alive){setStatus(e.payload);if(!e.payload.error)setError('');}},{target:'main'});
      if(!alive){off();return;}remove=off;const requested=revision;
      const s=await invoke<UpdateStatus>('updater_status');if(alive&&revision===requested&&s)setStatus(s);
    })().catch(()=>{});
    return()=>{alive=false;remove?.();};
  },[]);
  const check=async()=>{setError('');try{const s=await invoke<UpdateStatus>('updater_check');if(s)setStatus(s);}catch(e){setError(String(e));}};
  const install=async()=>{setError('');try{await invoke('updater_install');}catch(e){setError(String(e));}};
  return {status,error,check,install};
}
export default function UpdatesPanel({updater}:{updater:ReturnType<typeof useUpdates>}){
  const {status,error,check,install}=updater;
  const busy=['checking','downloading','verifying','installing'].includes(status.phase);
  const progress=status.total?Math.min(100,Math.round(status.downloaded/status.total*100)):undefined;
  const stages=['downloading','verifying','installing'];
  const upstream=status.upstream;
  const [linkError,setLinkError]=useState('');
  const openUpstream=async()=>{setLinkError('');try{await invoke('updater_open_upstream');}catch(e){setLinkError(String(e));}};
  const stage=stages.indexOf(status.phase);
  const megabytes=(bytes:number)=>`${(bytes/1048576).toLocaleString('es-AR',{maximumFractionDigits:1})} MB`;
  return <div className="updates-panel" data-updating={stage>=0}>
    <div className="form-row"><div><h2>Whispera</h2><p>Versión instalada: {status.currentVersion||'Vista previa'} · SikJa/Whispera</p></div><button disabled={!native||busy} onClick={()=>void check()}><RefreshCw size={16}/>{status.phase==='checking'?'Buscando…':'Buscar actualizaciones'}</button></div>
    {status.version&&<div className="update-card"><h3>Disponible: {status.version}</h3><p>Conservamos tus atajos, configuración e historial.</p>{status.notes&&<div className="update-notes">{briefUpdateNotes(status.notes)}</div>}<button className="update-install" disabled={busy} onClick={()=>void install()}><Download size={17}/>{status.phase==='installing'?'Instalando…':status.phase==='verifying'?'Verificando…':status.phase==='downloading'?'Descargando…':'Actualizar y reiniciar'}</button></div>}
    {status.phase==='current'&&<p role="status">Ya tenés la última versión.</p>}
    {status.phase==='idle'&&<p>Whispera comprueba si hay una versión nueva al iniciar y cada cinco minutos. Vos elegís cuándo instalarla.</p>}
    {stage>=0&&<div className="update-progress-card" role="status" aria-live="polite">
      <div className="update-progress-heading"><span className="update-progress-icon">{status.phase==='verifying'?<ShieldCheck size={22}/>:<LoaderCircle size={22} className="update-spinner"/>}</span><div><strong>{['Descargando actualización','Verificando el instalador','Instalando actualización'][stage]}</strong><p>{['Podés seguir usando Whispera mientras se descarga.','Comprobando la firma y la integridad del archivo.','Whispera se cerrará y volverá a abrirse.'][stage]}</p></div>{status.phase==='downloading'&&progress!==undefined&&<span className="update-percentage">{progress}%</span>}</div>
      {status.phase==='downloading'&&<><progress aria-label="Descarga de actualización" value={progress} max={100}/><div className="update-download-size">{megabytes(status.downloaded)}{status.total?` de ${megabytes(status.total)}`:''}</div></>}
      <ol className="update-stages">{['Descargar','Verificar','Instalar'].map((label,i)=><li key={label} data-state={i<stage?'done':i===stage?'active':'pending'} aria-current={i===stage?'step':undefined}><span>{i<stage?<Check size={12}/>:i+1}</span>{label}</li>)}</ol>
    </div>}
    {(error||status.error)&&<p className="update-error" role="alert">{error||status.error}</p>}
    {stage<0&&<section className="upstream-updates" aria-label="Novedades de Whispera-K">
      <h3>Whispera-K <small>kazu00001/Whispera-K</small></h3>
      {upstream?.phase==='available'?<><p>Publicación con cambios todavía no integrados: <strong>{upstream.version}</strong></p>{upstream.notes&&<p className="upstream-notes">{briefUpdateNotes(upstream.notes)}</p>}<button disabled={!native||busy} onClick={()=>void openUpstream()}><ExternalLink size={16}/>Ver novedades de K</button><p>Esta publicación no se instala sobre tu Whispera. Conservás el logo Cristal y nuestros arreglos.</p></>:upstream?.phase==='current'?<p>La última publicación de K ya está incluida en el código integrado.</p>:upstream?.phase==='error'?<p className="update-error">{upstream.error}</p>:<p>Pendiente de comprobar.</p>}
      {upstream?.phase==='error'&&upstream.version&&<p>Última publicación consultada: {upstream.version}. No se pudo verificar de nuevo.</p>}
      {linkError&&<p className="update-error" role="alert">{linkError}</p>}
    </section>}
  </div>;
}
