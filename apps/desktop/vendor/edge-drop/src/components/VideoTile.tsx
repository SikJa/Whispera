import { useEffect, useState } from 'react';
import { Play, Film } from 'lucide-react';
import '../styles/media-tools.css';
import VideoTranscript from './VideoTranscript';

const pending = new Map<string, Promise<any>>();
export function VideoTile({ path }: { path: string }) {
  const [metadata, setMetadata] = useState<any>();
  const native = (window as any).__TAURI_INTERNALS__;
  useEffect(() => {
    let active = true;
    if (native) {
      if (!pending.has(path)) pending.set(path, native.invoke('library_media_info', { path }).catch(() => null));
      pending.get(path)!.then(value => { if (active && value) setMetadata({ ...value, poster: native.convertFileSrc(value.previewPath, 'asset') }); });
    }
    return () => { active = false; };
  }, [path]);
  const seconds = Math.floor(metadata?.duration || 0);
  return <div className="wh-video-tile">
    {metadata?.poster ? <img src={metadata.poster} alt="" draggable={false}/> : native ? <Film className="wh-video-placeholder" size={28}/> : <video src={(window as any).__previewFiles?.[path]} muted preload="metadata" onLoadedMetadata={e => setMetadata({duration:e.currentTarget.duration})}/>}
    <span className="wh-video-play"><Play size={17} fill="currentColor"/></span>
    <VideoTranscript path={path} compact/>
    {metadata?.duration != null && <span className="wh-video-time">{Math.floor(seconds / 60)}:{String(seconds % 60).padStart(2,'0')}</span>}
  </div>;
}
