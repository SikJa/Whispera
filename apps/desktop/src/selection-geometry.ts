export type SelectionRect = {x:number;y:number;width:number;height:number};
export type SelectionHandle = 'move'|'n'|'ne'|'e'|'se'|'s'|'sw'|'w'|'nw';
const clamp=(value:number,min:number,max:number)=>Math.max(min,Math.min(max,value));
export function adjustSelection(rect:SelectionRect,handle:SelectionHandle,dx:number,dy:number,bounds:{width:number;height:number}):SelectionRect {
  const minWidth=Math.min(16,bounds.width),minHeight=Math.min(16,bounds.height);
  if(handle==='move')return {...rect,x:clamp(rect.x+dx,0,bounds.width-rect.width),y:clamp(rect.y+dy,0,bounds.height-rect.height)};
  let left=rect.x,top=rect.y,right=rect.x+rect.width,bottom=rect.y+rect.height;
  if(handle.includes('w'))left=clamp(left+dx,0,right-minWidth);
  if(handle.includes('e'))right=clamp(right+dx,left+minWidth,bounds.width);
  if(handle.includes('n'))top=clamp(top+dy,0,bottom-minHeight);
  if(handle.includes('s'))bottom=clamp(bottom+dy,top+minHeight,bounds.height);
  return {x:left,y:top,width:right-left,height:bottom-top};
}
