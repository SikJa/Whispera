import { useEffect, useRef, useState } from 'react';
import { Play, Square, Check, Volume2, ArrowLeft, Download } from 'lucide-react';
import { MotionConfig } from 'motion/react';
import ControlledFolder from './ControlledFolder';
import './sound-lab.css';

export const soundPairs = [
  { id: 'pop', name: 'Pop', note: 'Corto y discreto', source: 'Handy · MIT' },
  { id: 'marimba', name: 'Marimba', note: 'Percusión cálida', source: 'Handy · MIT' },
  { id: 'cristal', name: 'Cristal', note: 'Dos notas de vidrio', source: 'Whispera · Original' },
  { id: 'gota', name: 'Gota', note: 'Líquido y ligero', source: 'Whispera · Original' },
  { id: 'madera', name: 'Madera', note: 'Seco y suave', source: 'Whispera · Original' },
  { id: 'seda', name: 'Seda', note: 'Entrada gradual', source: 'Whispera · Original' },
  { id: 'pulso', name: 'Pulso', note: 'Digital y preciso', source: 'Whispera · Original' },
  { id: 'orbita', name: 'Órbita', note: 'Deslizamiento tonal', source: 'Whispera · Original' },
  { id: 'tecla', name: 'Tecla', note: 'Un toque mínimo', source: 'Whispera · Original' },
  { id: 'destello', name: 'Destello', note: 'Tres notas brillantes', source: 'Whispera · Original' },
];
type Cue = 'start' | 'stop' | 'pair';
const file = (id: string, cue: 'start' | 'stop') => `/sound-lab/${id}-${cue}.wav`;
export default function SoundLab() {
  const [selected, setSelected] = useState(() => {
    try { const id = localStorage.getItem('whispera.sound-lab.choice'); return soundPairs.some(p => p.id === id) ? id! : 'cristal'; } catch { return 'cristal'; }
  });
  const [volume, setVolume] = useState(.35);
  const [active, setActive] = useState('');
  const [phase, setPhase] = useState<'idle' | 'start' | 'stop'>('idle');
  const [error, setError] = useState('');
  const ctx = useRef<AudioContext | null>(null);
  const output = useRef<GainNode | null>(null);
  const nodes = useRef<AudioBufferSourceNode[]>([]);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const serial = useRef(0);
  const buffers = useRef(new Map<string, AudioBuffer>());
  function stop() {
    serial.current++;
    nodes.current.forEach(n => { try { n.stop(); } catch { /* Already ended. */ } n.disconnect(); });
    nodes.current = []; timers.current.forEach(clearTimeout); timers.current = [];
    setActive(''); setPhase('idle');
  }
  useEffect(() => () => { serial.current++; nodes.current.forEach(n => { try { n.stop(); } catch { /* Already ended. */ } }); timers.current.forEach(clearTimeout); void ctx.current?.close(); }, []);
  useEffect(() => { output.current?.gain.setTargetAtTime(volume, ctx.current!.currentTime, .02); }, [volume]);
  function choose(id: string) { setSelected(id); try { localStorage.setItem('whispera.sound-lab.choice', id); } catch { /* Preview remains usable without storage. */ } }
  async function play(id: string, cue: Cue) {
    stop(); choose(id); setError(''); const token = serial.current;
    try {
      const audio = ctx.current ?? (ctx.current = new AudioContext());
      if (!output.current) { output.current = audio.createGain(); output.current.gain.value = volume; output.current.connect(audio.destination); }
      await audio.resume();
      const load = async (kind: 'start' | 'stop') => {
        const url = file(id, kind); let buffer = buffers.current.get(url);
        if (!buffer) { const response = await fetch(url); if (!response.ok) throw Error('No se pudo cargar el sonido.'); buffer = await audio.decodeAudioData(await response.arrayBuffer()); buffers.current.set(url, buffer); }
        return buffer;
      };
      const first = await load(cue === 'stop' ? 'stop' : 'start');
      const second = cue === 'pair' ? await load('stop') : null;
      if (serial.current !== token) return;
      const schedule = (buffer: AudioBuffer, delay: number) => {
        const node = audio.createBufferSource(); node.buffer = buffer; node.connect(output.current!);
        nodes.current.push(node); node.start(audio.currentTime + .02 + delay);
        node.onended = () => node.disconnect();
      };
      setActive(`${id}:${cue}`); setPhase(cue === 'stop' ? 'stop' : 'start'); schedule(first, 0);
      let duration = first.duration;
      if (second) {
        const gap = Math.max(1.4, first.duration + .55); schedule(second, gap); duration = gap + second.duration;
        timers.current.push(setTimeout(() => setPhase('stop'), gap * 1000));
      }
      timers.current.push(setTimeout(() => { if (token === serial.current) { setActive(''); setPhase('idle'); nodes.current = []; } }, duration * 1000 + 80));
    } catch (e) { if (token === serial.current) { stop(); setError(String(e)); } }
  }
  const pair = soundPairs.find(p => p.id === selected)!;
  return <MotionConfig reducedMotion="user"><main className="sound-lab">
    <header className="sound-top"><a href="/"><ArrowLeft size={16}/><img src="/cristal/32x32.png" alt=""/>Whispera</a><span>Laboratorio de sonido · Preview</span></header>
    <div className="sound-layout"><section className="sound-library" aria-label="Opciones de sonido">
      <div className="sound-heading"><h1>Sonidos</h1><span>10 pares</span></div>
      <div className="sound-volume"><Volume2 size={17}/><label htmlFor="cue-volume">Volumen</label><input id="cue-volume" type="range" min="0" max="1" step=".01" value={volume} onChange={e => setVolume(Number(e.target.value))}/><output>{Math.round(volume * 100)}%</output></div>
      <div className="sound-rows">{soundPairs.map((item, i) => <div className="sound-row" data-selected={selected === item.id} key={item.id}>
        <label className="sound-choice"><input type="radio" name="sound-pair" checked={selected === item.id} onChange={() => { stop(); choose(item.id); }}/><span className="sound-number">{String(i + 1).padStart(2, '0')}</span><span><strong>{item.name}</strong><small>{item.note}</small></span></label>
        <div className="sound-auditions">{(['start', 'stop', 'pair'] as Cue[]).map(cue => <button key={cue} title={`${item.name}: ${cue === 'start' ? 'inicio' : cue === 'stop' ? 'fin' : 'par completo'}`} aria-label={`${item.name}: ${cue}`} aria-pressed={active === `${item.id}:${cue}`} onClick={() => active === `${item.id}:${cue}` ? stop() : void play(item.id, cue)}>{active === `${item.id}:${cue}` ? <Square size={13}/> : <Play size={13}/>}<span>{cue === 'start' ? 'Inicio' : cue === 'stop' ? 'Fin' : 'Par'}</span></button>)}</div>
      </div>)}</div>
    </section><aside className="sound-stage" aria-label="Sonido seleccionado">
      <div className="sound-folder"><ControlledFolder color="black" customColor="#9024DC" size="sm" visualState={phase === 'start' ? 'hover' : 'rest'}/></div>
      <div className="sound-status" key={phase} role="status"><i data-phase={phase}/>{phase === 'start' ? 'Inicio' : phase === 'stop' ? 'Fin de grabación' : 'En reposo'}</div>
      <h2>{pair.name}</h2><p>{pair.source}</p>
      <button className="sound-main-play" onClick={() => active ? stop() : void play(selected, 'pair')}>{active ? <Square size={17}/> : <Play size={17}/>} {active ? 'Detener' : 'Escuchar el par'}</button>
      <span className="sound-favorite"><Check size={14}/>Elección de preview guardada</span>
      <div className="sound-downloads"><a href={file(selected, 'start')} download><Download size={14}/>Inicio.wav</a><a href={file(selected, 'stop')} download><Download size={14}/>Fin.wav</a></div>
      {error && <p role="alert" className="sound-error">{error}</p>}
    </aside></div>
    <footer className="sound-sources"><a href="/sound-lab/SOURCES.md" target="_blank" rel="noreferrer">Créditos y referencias</a><a href="https://github.com/cjpais/Handy" target="_blank" rel="noreferrer">Handy</a><a href="https://docs.wisprflow.ai/articles/6409258247-starting-your-first-dictation" target="_blank" rel="noreferrer">Wispr Flow</a><a href="https://ai.superwhisper.com/changelog" target="_blank" rel="noreferrer">Superwhisper</a></footer>
  </main></MotionConfig>;
}
