import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen,type UnlistenFn} from '@tauri-apps/api/event';
import {Download,RefreshCw} from 'lucide-react';
import {native} from './client';
import './updates.css';

export type UpdateStatus={currentVersion:string;version:string|null;notes:string;phase:string;downloaded:number;total:number|null;error:string};
const initial:UpdateStatus={currentVersion:'',version:null,notes:'',phase:'idle',downloaded:0,total:null,error:''};
export function useUpdates(){
  const [status,setStatus]=useState(initial),[error,setError]=useState('');
  useEffect(()=>{
    if(!native)return;
    let alive=true,revision=0,remove:UnlistenFn|undefined;
    void(async()=>{
      const off=await listen<UpdateStatus>('updater-status',e=>{revision++;if(alive)setStatus(e.payload);},{target:'main'});
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
  const busy=['checking','downloading','installing'].includes(status.phase);
  const progress=status.total?Math.min(100,Math.round(status.downloaded/status.total*100)):undefined;
  return <div className="updates-panel">
    <div className="form-row"><div><h2>Whispera (K)</h2><p>Versión instalada: {status.currentVersion||'Vista previa'}</p></div><button disabled={!native||busy} onClick={()=>void check()}><RefreshCw size={16}/>{status.phase==='checking'?'Buscando…':'Buscar actualizaciones'}</button></div>
    {status.version&&<div className="update-card"><h3>Disponible: {status.version}</h3><p>Actualizá desde acá. Tus atajos, configuración e historial se conservan.</p>{status.notes&&<details><summary>Qué cambió</summary><div className="update-notes">{status.notes}</div></details>}<button className="update-install" disabled={busy} onClick={()=>void install()}><Download size={17}/>{status.phase==='installing'?'Instalando…':status.phase==='downloading'?'Descargando…':'Actualizar y reiniciar'}</button></div>}
    {status.phase==='current'&&<p role="status">Ya tenés la última versión.</p>}
    {status.phase==='idle'&&<p>Whispera comprueba si hay una versión nueva al iniciar y cada seis horas. Vos elegís cuándo instalarla.</p>}
    {status.phase==='downloading'&&<div role="status"><progress aria-label="Descarga de actualización" value={progress} max={100}/><span>{progress===undefined?'Descargando actualización…':`${progress}%`}</span></div>}
    {status.phase==='installing'&&<p role="status">Instalando. Whispera se cerrará y volverá a abrirse.</p>}
    {(error||status.error)&&<p className="update-error" role="alert">{error||status.error}</p>}
  </div>;
}
