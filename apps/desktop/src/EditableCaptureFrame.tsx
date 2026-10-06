import {useEffect,useRef,useState,type PointerEvent} from 'react';
import {invoke} from '@tauri-apps/api/core';
import CaptureFrame from './CaptureFrame';
import {adjustSelection,type SelectionRect,type SelectionHandle} from './selection-geometry';
export type CaptureContext={id:string;kind:'image'|'video';width:number;height:number;scale:number;hud_scale?:number;hud_above?:boolean;frame_color?:string;rect?:SelectionRect;monitor_width?:number;monitor_height?:number;source_label?:string};
const handles:SelectionHandle[]=['n','ne','e','se','s','sw','w','nw'];
export default function EditableCaptureFrame({context}:{context:CaptureContext}) {
  const [rect,setRect]=useState(context.rect!),[busy,setBusy]=useState(false),[error,setError]=useState('');
  const drag=useRef<{handle:SelectionHandle;rect:SelectionRect;x:number;y:number}|undefined>(undefined);
  const current=useRef(context.rect!),alive=useRef(true);
  const preview=useRef<{frame:number;pending?:SelectionRect;running?:Promise<void>;started?:Promise<unknown>}>({frame:0});
  const flushPreview=():Promise<void>=>{
    const state=preview.current;
    if(state.running)return state.running;
    const run=(async()=>{
      await state.started;
      while(alive.current&&state.pending){
        const next=state.pending;state.pending=undefined;
        await invoke('screen_frame_preview',{id:context.id,rect:next});
      }
    })();
    state.running=run;
    void run.finally(()=>{state.running=undefined;}).catch(()=>{});
    return run;
  };
  const queuePreview=(next:SelectionRect)=>{
    const state=preview.current;state.pending=next;
    if(state.frame)return;
    state.frame=requestAnimationFrame(()=>{state.frame=0;void flushPreview().catch(e=>{if(alive.current)setError(String(e));});});
  };
  useEffect(()=>{current.current=context.rect!;setRect(context.rect!);},[context.rect?.x,context.rect?.y,context.width,context.height]);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;cancelAnimationFrame(preview.current.frame);preview.current.pending=undefined;};},[]);
  const start=(e:PointerEvent<HTMLButtonElement>,handle:SelectionHandle)=>{
    if(busy||e.button!==0)return;e.preventDefault();e.stopPropagation();e.currentTarget.setPointerCapture(e.pointerId);
    drag.current={handle,rect:current.current,x:e.clientX,y:e.clientY};setError('');
    preview.current.started=invoke('screen_frame_drag',{active:true});
    void preview.current.started.catch(e=>{if(alive.current)setError(String(e));});
  };
  const move=(e:PointerEvent<HTMLButtonElement>)=>{
    const d=drag.current;if(!d)return;
    current.current=adjustSelection(d.rect,d.handle,e.clientX-d.x,e.clientY-d.y,{width:context.monitor_width!,height:context.monitor_height!});
    setRect(current.current);
    queuePreview(current.current);
  };
  const finish=async(cancel=false)=>{
    const d=drag.current;if(!d)return;drag.current=undefined;
    setBusy(true);
    try {
      if(cancel){current.current=d.rect;setRect(d.rect);}
      cancelAnimationFrame(preview.current.frame);preview.current.frame=0;
      preview.current.pending=current.current;
      await flushPreview();
      if(!cancel&&JSON.stringify(current.current)!==JSON.stringify(d.rect)){
        await invoke('screen_resize_region',{id:context.id,rect:current.current});
        if(context.kind==='video'){
          for(let attempt=0;attempt<100&&alive.current;attempt++){
            const status=await invoke<{phase:string}>('screen_status');
            if(status.phase!=='reframing')break;
            await new Promise(resolve=>setTimeout(resolve,50));
          }
        }
      }
    }catch(e){if(alive.current){current.current=d.rect;setRect(d.rect);setError(String(e));}}
    finally{await invoke('screen_frame_drag',{active:false}).catch(()=>{});if(alive.current)setBusy(false);}
  };
  const props=(handle:SelectionHandle)=>({disabled:busy,onPointerDown:(e:PointerEvent<HTMLButtonElement>)=>start(e,handle),onPointerMove:move,onPointerUp:()=>void finish(),onPointerCancel:()=>void finish(true)});
  return <div className="screen-indicator" aria-label="Área de captura editable">
    <CaptureFrame width={rect.width} height={rect.height} color={context.frame_color} image={context.kind==='image'} testId="recording-frame" className="editable-capture-frame" style={{left:rect.x,top:rect.y}}>
      <button className="capture-frame-move" aria-label="Mover área de captura" title="Arrastrá para mover el área" {...props('move')}/>
      {handles.map(handle=><button key={handle} className={`capture-frame-handle capture-frame-handle-${handle}`} aria-label={`Cambiar tamaño ${handle}`} title="Arrastrá para cambiar el tamaño" {...props(handle)}/>)}
    </CaptureFrame>
    {error&&<div className="capture-frame-error" role="alert" style={{left:rect.x,top:Math.max(0,rect.y-32)}}>{error}</div>}
  </div>;
}
