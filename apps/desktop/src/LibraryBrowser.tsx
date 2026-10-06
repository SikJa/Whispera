import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { Scissors, ClipboardList, Copy, File, FolderOpen, Image, Link, Pencil, Pin, PinOff, RefreshCw, Search, Trash2, Type, Video } from 'lucide-react';
import { native } from './client';

type LibraryImage = { imageId: string; width?: number; height?: number };
type LibraryItem = {
  id: string; capturedAt: number; pinned: boolean;
  data: { kind: string; text?: string; imageId?: string; images?: LibraryImage[]; paths?: string[]; entries?: { name: string }[] };
};
type LibraryState = { items?: LibraryItem[]; revision?: string; unchanged?: boolean };
type Kind = 'text' | 'link' | 'image' | 'video' | 'files';
const filters: { value: 'all' | Kind; label: string }[] = [
  { value: 'all', label: 'Todos' }, { value: 'text', label: 'Textos' },
  { value: 'link', label: 'Enlaces' }, { value: 'image', label: 'Imágenes' },
  { value: 'video', label: 'Videos' }, { value: 'files', label: 'Archivos' },
];
const icons = { text: Type, link: Link, image: Image, video: Video, files: File };
const basename = (path: string) => path.split(/[\\/]/).pop() || path;
function kindOf(item: LibraryItem): Kind {
  const data = item.data;
  if (data.kind === 'image' || data.kind === 'image-collection') return 'image';
  if (data.kind === 'text') return /^https?:\/\/\S+$/i.test(data.text?.trim() || '') ? 'link' : 'text';
  // A mixed file group stays in Files so its non-video attachments remain visible.
  if (data.paths?.length && data.paths.every(path => /\.(mp4|webm|mov|mkv|avi)$/i.test(path))) return 'video';
  return 'files';
}
function labelOf(item: LibraryItem) {
  return item.data.text || item.data.entries?.map(entry => entry.name).join(', ')
    || item.data.paths?.map(basename).join(', ') || (item.data.imageId ? basename(item.data.imageId) : '')
    || (item.data.images ? `${item.data.images.length} imágenes` : 'Elemento');
}
function Thumbnail({ path }: { path: string }) {
  const [failed, setFailed] = useState(false);
  return failed ? <Image size={22}/> : <img src={convertFileSrc(path)} alt="" loading="lazy" draggable={false} onError={() => setFailed(true)}/>;
}

export default function LibraryBrowser() {
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [loading, setLoading] = useState(native);
  const [loadError, setLoadError] = useState('');
  const [error, setError] = useState('');
  const [status, setStatus] = useState('');
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<'all' | Kind>('all');
  const [limit, setLimit] = useState(50);
  const [deleting, setDeleting] = useState<string | null>(null);
  const mounted = useRef(false);
  const revision = useRef<string | null>(null);
  const sequence = useRef(0);
  const pending = useRef(0);
  const acting = useRef(false);
  const refresh = useCallback(async (force = false) => {
    if (!native || (!force && (pending.current > 0 || acting.current))) return;
    const request = ++sequence.current;
    pending.current++;
    try {
      const state = await invoke<LibraryState>('library_state', { revision: force ? null : revision.current });
      if (!mounted.current || request !== sequence.current) return;
      if (!state || (!state.unchanged && !Array.isArray(state.items))) throw new Error('No se pudo leer la biblioteca.');
      if (!state.unchanged) {
        setItems(state.items!);
        revision.current = state.revision || null;
      }
      setLoadError('');
    } catch (cause) {
      if (mounted.current && request === sequence.current) setLoadError(String(cause));
    } finally {
      pending.current--;
      if (mounted.current && request === sequence.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    void refresh(true);
    const update = () => { if (document.visibilityState === 'visible') void refresh(); };
    const timer = native ? window.setInterval(update, 3000) : undefined;
    window.addEventListener('focus', update);
    document.addEventListener('visibilitychange', update);
    return () => {
      mounted.current = false;
      sequence.current++;
      window.clearInterval(timer);
      window.removeEventListener('focus', update);
      document.removeEventListener('visibilitychange', update);
    };
  }, [refresh]);
  const run = async (operation: () => Promise<unknown>, message: string, mutation = false) => {
    if (acting.current || !native) return;
    acting.current = true;
    setBusy(true); setError(''); setStatus('');
    try {
      await operation();
      if (mounted.current) { setStatus(message); setDeleting(null); }
      if (mutation) await refresh(true);
    } catch (cause) {
      if (mounted.current) setError(String(cause));
    } finally {
      acting.current = false;
      if (mounted.current) setBusy(false);
    }
  };
  const action = (item: LibraryItem, name: 'copy' | 'pin' | 'delete' | 'reveal') => void run(
    () => invoke('library_action', { action: name, id: item.id, value: name === 'pin' ? !item.pinned : null }),
    name === 'copy' ? 'Copiado.' : name === 'pin' ? (item.pinned ? 'Elemento desfijado.' : 'Elemento fijado.') : name === 'delete' ? 'Eliminado del historial.' : 'Ubicación abierta.',
    name === 'pin' || name === 'delete',
  );
  const visible = useMemo(() => {
    const term = query.trim().toLocaleLowerCase();
    return items.filter(item => (filter === 'all' || kindOf(item) === filter)
      && (!term || [labelOf(item), ...(item.data.paths || []), item.data.imageId || '', ...(item.data.images?.map(image => image.imageId) || [])].join('\n').toLocaleLowerCase().includes(term)))
      .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.capturedAt - a.capturedAt);
  }, [items, query, filter]);
  return <div className="library-browser" aria-busy={loading || busy}>
    <div className="library-toolbar">
      <label className="library-search"><Search size={16}/><input type="search" aria-label="Buscar en biblioteca" placeholder="Buscar en biblioteca" value={query} onChange={event => { setQuery(event.target.value); setLimit(50); }}/></label>
      <button className="library-icon" title="Actualizar biblioteca" aria-label="Actualizar biblioteca" disabled={!native || busy || loading} onClick={() => void refresh(true)}><RefreshCw size={16}/></button>
      <button disabled={!native || busy} onClick={() => void run(() => invoke('library_toggle'), 'Estante alternado.')}><ClipboardList size={16}/>Abrir estante</button>
    </div>
    <div className="library-filters" role="group" aria-label="Tipo de elemento">
      {filters.map(option => <button key={option.value} aria-pressed={filter === option.value} onClick={() => { setFilter(option.value); setLimit(50); setDeleting(null); }}>{option.label}</button>)}
    </div>
    <div className="library-summary"><span>{visible.length} {visible.length === 1 ? 'elemento' : 'elementos'}</span><span role="status">{status}</span></div>
    {(error || loadError) && <p role="alert" className="notice">{error || loadError}</p>}
    {!native ? <p className="library-empty">La biblioteca local está disponible en la aplicación de escritorio.</p>
      : loading ? <p className="library-empty" role="status">Cargando biblioteca…</p>
      : !visible.length && !loadError ? <p className="library-empty">{items.length ? 'Sin coincidencias.' : 'La biblioteca está vacía.'}</p> : null}
    <ul className="library-list" aria-label="Elementos de biblioteca">
      {visible.slice(0, limit).map(item => {
        const kind = kindOf(item), Icon = icons[kind];
        const imagePath = item.data.imageId || item.data.images?.[0]?.imageId;
        const hasFile = Boolean(imagePath || item.data.paths?.length);
        const date = new Date(item.capturedAt);
        return <li className="library-item" key={item.id} data-library-id={item.id}>
          <div className={`library-thumb library-kind-${kind}`}>{imagePath ? <Thumbnail key={imagePath} path={imagePath}/> : <Icon size={22}/>}</div>
          <div className="library-item-copy"><p title={labelOf(item)}>{labelOf(item)}</p><small>{item.pinned && <Pin size={12} aria-label="Fijado"/>}{filters.find(option => option.value === kind)?.label}{!Number.isNaN(date.getTime()) && <time dateTime={date.toISOString()}>{date.toLocaleString('es-AR', { dateStyle: 'short', timeStyle: 'short' })}</time>}</small></div>
          <div className="library-item-actions">
            <button className="library-icon" title="Copiar" aria-label="Copiar" disabled={busy} onClick={() => action(item, 'copy')}><Copy size={16}/></button>
            <button className="library-icon" title={item.pinned ? 'Desfijar' : 'Fijar'} aria-label={item.pinned ? 'Desfijar' : 'Fijar'} aria-pressed={item.pinned} disabled={busy} onClick={() => action(item, 'pin')}>{item.pinned ? <PinOff size={16}/> : <Pin size={16}/>}</button>
            {(kind === 'image' || item.data.paths?.some(path => /\.(png|jpe?g|webp)$/i.test(path))) && <button className="library-icon" title="Editar imagen" aria-label="Editar imagen" disabled={busy} onClick={() => void run(() => invoke('library_media_open', { id: item.id, mode: 'canvas' }), 'Editor abierto.')}><Pencil size={16}/></button>}
            {(kind === 'video' || item.data.paths?.some(path => /\.(mp4|mov|mkv|webm)$/i.test(path))) && <button className="library-icon" title="Recortar video" aria-label="Recortar video" disabled={busy} onClick={() => void run(() => invoke('video_trim_open', { id: item.id }), 'Editor abierto.')}><Scissors size={16}/></button>}
            {hasFile && <button className="library-icon" title="Abrir ubicación" aria-label="Abrir ubicación" disabled={busy} onClick={() => action(item, 'reveal')}><FolderOpen size={16}/></button>}
            <button className="library-icon" title="Eliminar del historial" aria-label="Eliminar del historial" disabled={busy} onClick={() => setDeleting(item.id)}><Trash2 size={16}/></button>
          </div>
          {deleting === item.id && <div className="library-delete" role="group" aria-label="Confirmar eliminación"><span>¿Eliminar del historial?</span><button disabled={busy} onClick={() => setDeleting(null)}>Cancelar</button><button disabled={busy} onClick={() => action(item, 'delete')}>Eliminar</button></div>}
        </li>;
      })}
    </ul>
    {visible.length > limit && <button className="library-more" onClick={() => setLimit(value => value + 50)}>Mostrar más</button>}
  </div>;
}
