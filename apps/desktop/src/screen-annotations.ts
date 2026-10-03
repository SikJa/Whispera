export type Shape = 'rectangle' | 'ellipse' | 'triangle' | 'diamond' | 'hexagon' | 'star';
export type Tool = 'pointer' | 'pen' | 'line' | 'arrow' | Shape | 'highlight' | 'text' | 'blur';
export const isShape = (tool: Tool): tool is Shape => ['rectangle','ellipse','triangle','diamond','hexagon','star'].includes(tool);
export type Point = { x: number; y: number };
export type Mark = { id: string; tool: Tool; color: string; width: number; points: Point[]; text?: string; bitmap?: HTMLCanvasElement };
export type History = { past: Mark[][]; present: Mark[]; future: Mark[][] };
export const emptyHistory = (): History => ({ past: [], present: [], future: [] });
export function commit(history: History, marks: Mark[]): History {
  return { past: [...history.past.slice(-79), history.present], present: marks, future: [] };
}
export function undo(history: History): History {
  if (!history.past.length) return history;
  return { past: history.past.slice(0, -1), present: history.past.at(-1)!, future: [history.present, ...history.future] };
}
export function redo(history: History): History {
  if (!history.future.length) return history;
  return { past: [...history.past, history.present], present: history.future[0], future: history.future.slice(1) };
}
export function constrained(start: Point, end: Point, tool: Tool, shift: boolean): Point {
  if (!shift) return end;
  const dx = end.x - start.x, dy = end.y - start.y;
  if (isShape(tool) || tool === 'blur') {
    const size = Math.max(Math.abs(dx), Math.abs(dy));
    return { x: start.x + Math.sign(dx || 1) * size, y: start.y + Math.sign(dy || 1) * size };
  }
  if (tool === 'line' || tool === 'arrow') {
    const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI / 4;
    const length = Math.hypot(dx, dy);
    return { x: start.x + Math.cos(angle) * length, y: start.y + Math.sin(angle) * length };
  }
  return end;
}
export function bounds(mark: Mark) {
  const xs = mark.points.map(p => p.x), ys = mark.points.map(p => p.y);
  const x = Math.min(...xs), y = Math.min(...ys);
  return { x, y, width: mark.tool === 'text' ? Math.max(24, (mark.text?.length ?? 1) * (14 + mark.width * 3) * .62) : Math.max(...xs) - x,
    height: mark.tool === 'text' ? 18 + mark.width * 3 : Math.max(...ys) - y };
}
export function hitTest(marks: Mark[], point: Point) {
  return [...marks].reverse().find(mark => {
    const b = bounds(mark), margin = Math.max(7, mark.width);
    return point.x >= b.x - margin && point.x <= b.x + b.width + margin && point.y >= b.y - margin && point.y <= b.y + b.height + margin;
  });
}
export function paint(ctx: CanvasRenderingContext2D, mark: Mark) {
  const first = mark.points[0], last = mark.points.at(-1)!;
  if (!first) return;
  ctx.save(); ctx.strokeStyle = mark.color; ctx.fillStyle = mark.color;
  ctx.lineWidth = mark.width; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  if (mark.tool === 'text') {
    ctx.font = `600 ${14 + mark.width * 3}px "Segoe UI", sans-serif`; ctx.textBaseline = 'top';
    ctx.fillText(mark.text ?? '', first.x, first.y);
  } else if (isShape(mark.tool) || mark.tool === 'blur') {
    const b = bounds(mark);
    if (mark.tool === 'blur' && mark.bitmap) {
      ctx.beginPath();ctx.roundRect(b.x,b.y,b.width,b.height,Math.min(12,b.width/4,b.height/4));ctx.clip();
      ctx.drawImage(mark.bitmap,b.x,b.y,b.width,b.height);
    }
    else if (mark.tool === 'rectangle' || mark.tool === 'blur') ctx.strokeRect(b.x, b.y, b.width, b.height);
    else {
      ctx.beginPath();
      if (mark.tool === 'ellipse') ctx.ellipse(b.x+b.width/2,b.y+b.height/2,b.width/2,b.height/2,0,0,Math.PI*2);
      else {
        const vertices = mark.tool === 'triangle' ? [[.5,0],[1,1],[0,1]] : mark.tool === 'diamond' ? [[.5,0],[1,.5],[.5,1],[0,.5]] : Array.from({length:mark.tool==='star'?10:6},(_,i)=>{
          const angle=i*Math.PI*2/(mark.tool==='star'?10:6)-Math.PI/2, radius=mark.tool==='star'&&i%2?.43:1;
          return [.5+Math.cos(angle)*.5*radius,.5+Math.sin(angle)*.5*radius];
        });
        vertices.forEach(([x,y],i)=>i?ctx.lineTo(b.x+x*b.width,b.y+y*b.height):ctx.moveTo(b.x+x*b.width,b.y+y*b.height));ctx.closePath();
      }
      ctx.stroke();
    }
  } else {
    if (mark.tool === 'highlight') { ctx.globalAlpha = .35; ctx.lineWidth = mark.width * 6; ctx.lineCap = 'butt'; }
    ctx.beginPath(); ctx.moveTo(first.x, first.y);
    for (const p of mark.points.slice(1)) ctx.lineTo(p.x, p.y);
    if (mark.points.length === 1) ctx.lineTo(first.x + .01, first.y);
    ctx.stroke();
    if (mark.tool === 'arrow') {
      const angle = Math.atan2(last.y - first.y, last.x - first.x), head = Math.max(12, mark.width * 4);
      ctx.beginPath(); ctx.moveTo(last.x, last.y);
      ctx.lineTo(last.x - head * Math.cos(angle - .48), last.y - head * Math.sin(angle - .48));
      ctx.lineTo(last.x - head * Math.cos(angle + .48), last.y - head * Math.sin(angle + .48));
      ctx.closePath(); ctx.fill();
    }
  }
  ctx.restore();
}
export function blurredTile(source:CanvasImageSource,width:number,height:number,scale:number,sourceRect?:{x:number;y:number;width:number;height:number}) {
  const tile=document.createElement('canvas');tile.width=Math.max(1,Math.round(width*scale));tile.height=Math.max(1,Math.round(height*scale));
  const ctx=tile.getContext('2d')!;ctx.fillStyle='#74747a';ctx.fillRect(0,0,tile.width,tile.height);
  ctx.filter=`blur(${18*scale}px)`;
  if(sourceRect)ctx.drawImage(source,sourceRect.x*scale,sourceRect.y*scale,sourceRect.width*scale,sourceRect.height*scale,-12*scale,-12*scale,tile.width+24*scale,tile.height+24*scale);
  else ctx.drawImage(source,-12*scale,-12*scale,tile.width+24*scale,tile.height+24*scale);
  return tile;
}
