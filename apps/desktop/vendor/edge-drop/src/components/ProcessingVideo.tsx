import { Film, FolderOpen, Trash2 } from 'lucide-react';
import type { ClipboardItemDto } from '../../shared/types';
import { useStore } from '../store/appStore';
import GridReveal from './grid-reveal';
import '../styles/processing-video.css';

export function ProcessingVideo({item}:{item:ClipboardItemDto}) {
  const open = useStore(s => s.open);
  const processing = item.data.kind === 'files' ? item.data.processing! : null;
  if (!processing) return null;
  const failed = processing.status === 'error';
  const seconds = Math.floor(processing.seconds || 0);
  return <div className="item wh-processing-card" data-processing={processing.status}>
    <div className="item-main" data-id={item.id}>
      {failed ? <div className="wh-processing-failed"><Film size={28}/></div> : <GridReveal src={null} active={open} aspect={16/9} caption="Preparando video" estimatedDuration={12000} alt="Video grabado, procesando MP4"/>}
      <div className="wh-processing-meta" role="status" aria-live="polite">
        <strong>{failed ? 'Video pendiente de recuperar' : 'Video grabado · Procesando'}</strong>
        <span>{Math.floor(seconds/60)}:{String(seconds%60).padStart(2,'0')} · {failed ? 'Originales conservados' : 'Preparando el archivo MP4'}</span>
      </div>
      {failed && <p className="wh-processing-error">{processing.error}</p>}
      <div className="wh-processing-actions">
        <button className="act" title="Abrir carpeta de recuperacion" aria-label="Abrir carpeta de recuperacion" onClick={() => void (window as any).__TAURI_INTERNALS__.invoke('library_action',{action:'reveal',id:item.id,value:null}).catch((error:unknown)=>useStore.getState().pushToast({id:crypto.randomUUID(),message:String(error),tone:'error'}))}><FolderOpen size={15}/></button>
        <button className="act" title="Quitar del historial" aria-label="Quitar del historial" onClick={() => useStore.getState().remove(item.id)}><Trash2 size={15}/></button>
      </div>
    </div>
  </div>;
}
