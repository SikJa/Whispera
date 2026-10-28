import React, { useEffect, useRef, useState } from 'react';
import { MousePointer2, Pencil, MoveUpRight, Minus, Square, Circle, Triangle, Highlighter, Type, Undo2, Redo2, Trash2 } from 'lucide-react';
import { commit, constrained, emptyHistory, paint, redo, undo, type Mark, type Tool } from '../../../src/screen-annotations';
import { presets } from '../../../src/palette';

export function useAnnotations() {
  const [history, setHistory] = useState(emptyHistory);
  const [tool, setTool] = useState<Tool>('pointer');
  const [color, setColor] = useState('#ff243b');
  const [width, setWidth] = useState(4);
  const [text, setText] = useState('');
  return { history, setHistory, tool, setTool, color, setColor, width, setWidth, text, setText };
}
type State = ReturnType<typeof useAnnotations>;
const tools = [
  ['pointer', 'Mover capturas', MousePointer2], ['arrow', 'Flecha', MoveUpRight],
  ['pen', 'Lapiz', Pencil], ['line', 'Linea', Minus], ['rectangle', 'Rectangulo', Square],
  ['ellipse', 'Circulo', Circle], ['triangle', 'Triangulo', Triangle],
  ['highlight', 'Resaltador', Highlighter], ['text', 'Texto', Type],
] as const;

export function AnnotationToolbar({ state: s }: { state: State }) {
  return <div className="annotation-toolbar" role="toolbar" aria-label="Anotar imagen">
    {tools.map(([tool, label, Icon]) => <button key={tool} className="media-tool" title={label} aria-label={label} aria-pressed={s.tool === tool} onClick={() => s.setTool(tool)}><Icon size={16}/></button>)}
    <label className="annotation-color" title="Color del trazo"><input type="color" aria-label="Color del trazo" list="annotation-colors" value={s.color} onChange={e => s.setColor(e.target.value)}/></label>
    <datalist id="annotation-colors">{presets.map(p => <option key={p.hex} value={p.hex}/>)}</datalist>
    <input className="annotation-width" type="range" min="1" max="12" aria-label="Grosor del trazo" title="Grosor del trazo" value={s.width} onChange={e => s.setWidth(Number(e.target.value))}/>
    <button className="media-tool" title="Deshacer trazo" aria-label="Deshacer trazo" disabled={!s.history.past.length} onClick={() => s.setHistory(undo)}><Undo2 size={16}/></button>
    <button className="media-tool" title="Rehacer trazo" aria-label="Rehacer trazo" disabled={!s.history.future.length} onClick={() => s.setHistory(redo)}><Redo2 size={16}/></button>
    <button className="media-tool" title="Borrar trazos" aria-label="Borrar trazos" disabled={!s.history.present.length} onClick={() => s.setHistory(h => commit(h, []))}><Trash2 size={16}/></button>
    {s.tool === 'text' && <input className="annotation-text" aria-label="Texto de la anotacion" value={s.text} onChange={e => s.setText(e.target.value)}/>}
  </div>;
}

export function AnnotationCanvas({ state: s, width, height }: { state: State; width: number; height: number }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const draft = useRef<Mark | null>(null);
  const draw = () => {
    const ctx = canvas.current?.getContext('2d'); if (!ctx) return;
    ctx.clearRect(0, 0, width, height);
    for (const mark of s.history.present) paint(ctx, mark);
    if (draft.current) paint(ctx, draft.current);
  };
  useEffect(() => { draft.current = null; draw(); }, [s.history, s.tool, width, height]);
  const point = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    return { x: Math.max(0, Math.min(width, (e.clientX - rect.left) * width / rect.width)), y: Math.max(0, Math.min(height, (e.clientY - rect.top) * height / rect.height)) };
  };
  return <canvas ref={canvas} className="annotation-canvas" width={width} height={height} aria-label="Lienzo de anotaciones" style={{ pointerEvents: s.tool === 'pointer' ? 'none' : 'auto' }}
    onPointerDown={e => {
      if (e.button !== 0) return;
      e.stopPropagation(); e.currentTarget.setPointerCapture(e.pointerId);
      const mark: Mark = { id: crypto.randomUUID(), tool: s.tool, color: s.color, width: s.width, points: [point(e)], text: s.text };
      if (s.tool === 'text') { if (s.text.trim()) s.setHistory(h => commit(h, [...h.present, mark])); return; }
      draft.current = mark; draw();
    }}
    onPointerMove={e => {
      const mark = draft.current; if (!mark) return;
      const p = constrained(mark.points[0], point(e), mark.tool, e.shiftKey);
      mark.points = mark.tool === 'pen' || mark.tool === 'highlight' ? [...mark.points, p] : [mark.points[0], p]; draw();
    }}
    onPointerUp={e => {
      const mark = draft.current; draft.current = null;
      if (mark) s.setHistory(h => commit(h, [...h.present, mark]));
      if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    }}
    onPointerCancel={() => { draft.current = null; draw(); }}
    onLostPointerCapture={() => { draft.current = null; draw(); }}/>
}
