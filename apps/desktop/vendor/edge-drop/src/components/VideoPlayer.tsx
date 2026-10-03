import { useRef, useState } from 'react'
import { Play, Pause, Volume2, VolumeX } from 'lucide-react'
import '../styles/video-player.css'

const timestamp = (seconds: number) => `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`

export default function VideoPlayer({ src }: { src: string }) {
  const ref = useRef<HTMLVideoElement>(null)
  const [playing, setPlaying] = useState(false)
  const [time, setTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [muted, setMuted] = useState(false)
  const [error, setError] = useState('')
  const toggle = async () => {
    const video = ref.current
    if (!video) return
    if (!video.paused) video.pause()
    else try { await video.play(); setError('') } catch { setError('No se pudo reproducir este video.') }
  }
  return <div className="library-video-player">
    <video ref={ref} src={src} preload="metadata" playsInline
      onLoadedMetadata={e => setDuration(Number.isFinite(e.currentTarget.duration) ? e.currentTarget.duration : 0)}
      onTimeUpdate={e => setTime(e.currentTarget.currentTime)}
      onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => setPlaying(false)}
      onError={() => setError('No se puede reproducir este formato. Abrilo desde su carpeta.')}
      onVolumeChange={e => setMuted(e.currentTarget.muted)} />
    <div className="library-video-controls">
      <button type="button" aria-label={playing ? 'Pausar video' : 'Reproducir video'} title={playing ? 'Pausar video' : 'Reproducir video'} onClick={() => void toggle()}>{playing ? <Pause size={16}/> : <Play size={16}/>}</button>
      <output>{timestamp(time)} / {timestamp(duration)}</output>
      <input type="range" aria-label="Posicion del video" min={0} max={duration || 1} step={0.1} value={Math.min(time, duration)} disabled={!duration} onChange={e => { if (ref.current) { ref.current.currentTime = Number(e.target.value); setTime(Number(e.target.value)) } }}/>
      <button type="button" aria-label={muted ? 'Activar sonido' : 'Silenciar video'} title={muted ? 'Activar sonido' : 'Silenciar video'} onClick={() => { if (ref.current) ref.current.muted = !ref.current.muted }}>{muted ? <VolumeX size={16}/> : <Volume2 size={16}/>}</button>
    </div>
    {error && <p role="alert">{error}</p>}
  </div>
}
