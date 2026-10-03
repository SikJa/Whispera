import { useEffect, useState } from 'react';
import { Copy, RotateCcw } from 'lucide-react';
type Transcript = {status:string;text:string;error:string};
export default function VideoTranscript({path,compact=false}:{path:string;compact?:boolean}) {
  const [value,setValue]=useState<Transcript>();
  const [message,setMessage]=useState('');
  const [revision,setRevision]=useState(0);
  useEffect(()=>{
    let active=true;let timer:ReturnType<typeof setTimeout>;let misses=0;
    const refresh=async()=>{
      try {const next=await (window as any).__TAURI_INTERNALS__?.invoke('video_transcript_action',{path,action:'status'});if(active)setValue(next);if(next?.status==='ready'||next?.status==='error')return;if(!next?.status&&++misses>=3)return;}
      catch { /* Keep video usable even if the transcript service is unavailable. */ }
      if(active)timer=setTimeout(refresh,2500);
    };
    void refresh();return()=>{active=false;clearTimeout(timer);};
  },[path,revision]);
  if(!value?.status)return null;
  const action=async(action:string)=>{try{const next=await(window as any).__TAURI_INTERNALS__.invoke('video_transcript_action',{path,action});setValue(next);if(action==='retry')setRevision(r=>r+1);setMessage(action==='copy'?'Texto copiado':'');}catch(e){setMessage(String(e));}};
  const label=value.status==='ready'?(value.text.trim()?'Voz transcrita':'Sin voz detectada'):value.status==='error'?'Error de transcripción':'Transcribiendo voz…';
  if(compact)return <span className="wh-video-transcript-state" title={value.error||label}>{label}</span>;
  return <section className="wh-video-transcript" aria-label="Transcripción del video">
    <strong>{label}</strong>
    {value.status==='ready'&&value.text&&<><textarea aria-label="Texto del video" readOnly value={value.text} rows={5}/><button onClick={()=>void action('copy')}><Copy size={14}/>Copiar texto</button></>}
    {value.status==='error'&&<><p role="alert">{value.error}</p><button onClick={()=>void action('retry')}><RotateCcw size={14}/>Reintentar</button></>}
    {message&&<p role="status">{message}</p>}
  </section>;
}
