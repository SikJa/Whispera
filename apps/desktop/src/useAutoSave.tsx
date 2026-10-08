import {useEffect,useRef,useState} from 'react';

// Serialize writes, coalesce fast edits, and flush a pending edit when leaving a page.
export function useAutoSave<T>(save:(value:T)=>Promise<unknown>) {
  const saveRef=useRef(save);saveRef.current=save;
  const pending=useRef<{value:T}|undefined>(undefined);const running=useRef(false);
  const timer=useRef<ReturnType<typeof setTimeout>|undefined>(undefined);const alive=useRef(true);
  const [status,setStatus]=useState('');const [error,setError]=useState(false);
  const report=(text:string,failed=false)=>{if(alive.current){setStatus(text);setError(failed);}};
  async function flush(){
    clearTimeout(timer.current);if(running.current)return;
    running.current=true;
    try{while(pending.current){const edit=pending.current;pending.current=undefined;
      try{await saveRef.current(edit.value);report(pending.current?'Guardando…':'Guardado automáticamente');}
      catch(e){if(pending.current)continue;pending.current=edit;report(`No se pudo guardar: ${String(e)}`,true);break;}
    }}finally{running.current=false;}
  }
  function queue(value:T){pending.current={value};report('Guardando…');clearTimeout(timer.current);timer.current=setTimeout(()=>void flush(),200);}
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;clearTimeout(timer.current);void flush();};},[]);
  return {queue,flush,status,error};
}
export function AutoSaveStatus({save}:{save:{status:string;error:boolean;flush:()=>Promise<void>}}){
  return save.status?<span role="status" className="muted-note">{save.status}{save.error&&<button onClick={()=>void save.flush()}>Reintentar</button>}</span>:null;
}
