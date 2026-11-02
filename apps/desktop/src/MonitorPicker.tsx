import { useEffect } from "react";
import { Monitor } from "lucide-react";

export type MonitorInfo = { index: number; name: string | null; x?: number; y?: number; width: number; height: number; primary?: boolean };

const MAP_WIDTH = 440, MAP_HEIGHT = 170, GAP = 6;

// Mouse clicks reach the selection surface (so a drag can start on a tile);
// the button itself only handles keyboard activation (detail 0).
// The monitors drawn as they are arranged in Windows/macOS display settings,
// so "which screen is which" is answered by position, not by a number.
export default function MonitorPicker({ monitors, current, busy, onPick }: { monitors: MonitorInfo[]; current: number | null; busy: boolean; onPick: (index: number) => void }) {
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (busy || e.ctrlKey || e.metaKey || e.altKey) return;
      const index = Number(e.key) - 1;
      if (Number.isInteger(index) && monitors.some(m => m.index === index)) { e.preventDefault(); onPick(index); }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [busy, monitors, onPick]);
  if (!monitors.length) return null;
  // Without real coordinates (older data, mocks) lay the screens side by side.
  const positioned = monitors.every(m => m.x !== undefined && m.y !== undefined);
  let cursor = 0;
  monitors = positioned ? monitors : monitors.map(m => { const next = { ...m, x: cursor, y: 0 }; cursor += m.width; return next; });
  const left = Math.min(...monitors.map(m => m.x ?? 0)), top = Math.min(...monitors.map(m => m.y ?? 0));
  const right = Math.max(...monitors.map(m => (m.x ?? 0) + m.width)), bottom = Math.max(...monitors.map(m => (m.y ?? 0) + m.height));
  const scale = Math.min(MAP_WIDTH / (right - left), MAP_HEIGHT / (bottom - top));
  const width = (right - left) * scale, height = (bottom - top) * scale;
  const label = (m: MonitorInfo) => m.name && !m.name.startsWith("\\\\.\\") ? m.name : `Pantalla ${m.index + 1}`;
  return <div className="monitor-picker" role="group" aria-label="Pantalla para grabar">
    <div className="monitor-picker-head">
      <span className="monitor-picker-icon"><Monitor size={18} /></span>
      <div><strong>¿Qué querés grabar?</strong><small>Arrastrá sobre la pantalla para elegir un área, o grabá una pantalla completa.</small></div>
    </div>
    <div className="monitor-map" style={{ width, height }}>
      {monitors.map(m => {
        const tile = { left: ((m.x ?? 0) - left) * scale + GAP / 2, top: ((m.y ?? 0) - top) * scale + GAP / 2, width: m.width * scale - GAP, height: m.height * scale - GAP };
        const roomy = tile.width > 120 && tile.height > 64;
        return <button key={m.index} disabled={busy} className="monitor-tile" data-index={m.index} data-current={m.index === current} data-roomy={roomy} style={tile}
          aria-label={`Pantalla ${m.index + 1}: ${label(m)}, ${m.width} × ${m.height}${m.index === current ? ", esta pantalla" : ""}`} onClick={e => { if (e.detail === 0) onPick(m.index); }}>
          <b className="monitor-number">{m.index + 1}</b>
          {roomy && <span className="monitor-name">{label(m)}</span>}
          {roomy && <small>{m.width} × {m.height}{m.primary ? " · Principal" : ""}</small>}
          {m.index === current && <em className="monitor-here">Esta pantalla</em>}
        </button>;
      })}
    </div>
    <p className="monitor-picker-keys">
      {monitors.length > 1 && <><kbd>1</kbd>{monitors.length > 2 ? "–" : "/"}<kbd>{monitors.length}</kbd> elegir pantalla</>}
      <span><kbd>Esc</kbd> salir</span>
    </p>
  </div>;
}
