import { useEffect, useRef } from 'react';
import { Play } from 'lucide-react';
import { SettingsSwitch } from './ResourceControls';
import { soundPairs } from './SoundLab';
import type { Settings } from './client';

export default function SoundSettings({ settings, patch, onError }: {
  settings: Settings;
  patch: (change: Partial<Settings>) => void;
  onError: (message: string) => void;
}) {
  const audio = useRef<HTMLAudioElement | null>(null);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const generation = useRef(0);
  function stop() {
    generation.current++;
    timers.current.forEach(clearTimeout);
    timers.current = [];
    audio.current?.pause();
    audio.current = null;
  }
  useEffect(() => stop, []);
  function preview(theme: string, cue: 'start' | 'stop') {
    stop();
    const token = generation.current;
    function play(kind: 'start' | 'stop') {
      if (generation.current !== token) return;
      audio.current?.pause();
      const sample = new Audio(`/sound-lab/${theme}-${kind}.wav`);
      sample.volume = .35;
      audio.current = sample;
      void sample.play().then(() => {
        if (generation.current !== token) sample.pause();
      }).catch(() => {
        if (generation.current === token) onError('No se pudo reproducir la muestra de sonido.');
      });
    }
    play(cue);
    timers.current.push(setTimeout(stop, 2000));
  }
  return <>
    <div className="form-row"><label htmlFor="sounds-on">Sonidos de grabación</label><SettingsSwitch id="sounds-on" label="Activar sonidos" checked={settings.sounds} onChange={sounds => patch({ sounds })}/></div>
    <div className="form-row"><label htmlFor="sound-theme">Inicio y fin<span>Usá los botones para escuchar una muestra. Los cambios se guardan solos.</span></label><select id="sound-theme" value={settings.soundTheme} onChange={e => {
      const soundTheme = e.target.value;
      patch({ soundTheme });
      stop();
    }}>{soundPairs.map(p => <option value={p.id} key={p.id}>{p.name}</option>)}</select></div>
    <div className="page-actions">{(['start', 'stop'] as const).map(cue => <button key={cue} onClick={() => preview(settings.soundTheme, cue)}><Play size={15}/>{cue === 'start' ? 'Escuchar inicio' : 'Escuchar fin'}</button>)}</div>
  </>;
}
