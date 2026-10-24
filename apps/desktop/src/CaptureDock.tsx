import {useEffect,useRef,useState,type PointerEvent,type RefObject} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {GripVertical,Minus,Plus} from 'lucide-react';

type View={zoom:number;compact:boolean};
type Gesture={x:number;y:number;pending?:{x:number;y:number};ready:Promise<unknown>;running?:Promise<void>};
export function useCaptureDock(id:string,element:RefObject<HTMLElement|null>,onError:(error:string)=>void,beforeDrag?:()=>Promise<unknown>) {
  const [view,setView]=useState<View>({zoom:1,compact:false});
  const gesture=useRef<Gesture|undefined>(undefined);const live=useRef(true);
  const request=(action:string,args:Record<string,unknown>={})=>invoke<View>('screen_overlay_customize',{id,action,...args});
  const change=async(values:Partial<View>)=>{try{const value=await request('configure',values);if(live.current&&value)setView(value);}catch(e){onError(String(e));}};
  useEffect(()=>{live.current=true;void request('get').then(value=>{if(live.current&&value)setView(value);}).catch(e=>onError(String(e)));return()=>{live.current=false;};},[id]);
  useEffect(()=>{
    const target=element.current;if(!target)return;
    let timer=0,previous='';
    const measure=()=>{
      const width=target.offsetWidth+8,height=target.offsetHeight+8,key=`${width}:${height}:${view.zoom}`;
      if(key===previous)return;previous=key;
      void request('size',{width,height}).catch(e=>onError(String(e)));
    };
    const observer=new ResizeObserver(()=>{cancelAnimationFrame(timer);timer=requestAnimationFrame(measure);});
    observer.observe(target);measure();return()=>{observer.disconnect();cancelAnimationFrame(timer);};
  },[id,view.zoom,view.compact,element]);
  const flush=(g:Gesture):Promise<void>=>{
    if(g.running)return g.running;
    const run=async()=>{await g.ready;while(g.pending){const point=g.pending;g.pending=undefined;await request('move',point);}};
    g.running=run().finally(()=>{g.running=undefined;});return g.running;
  };
  const start=(e:PointerEvent<HTMLButtonElement>)=>{
    if(e.button!==0||gesture.current)return;e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    const ready=(async()=>{await beforeDrag?.();await request('begin');})();
    gesture.current={x:e.screenX,y:e.screenY,ready};
    void ready.catch(e=>onError(String(e)));
  };
  const move=(e:PointerEvent<HTMLButtonElement>)=>{const g=gesture.current;if(!g)return;g.pending={x:e.screenX-g.x,y:e.screenY-g.y};void flush(g).catch(e=>onError(String(e)));};
  const finish=async(cancel=false)=>{const g=gesture.current;if(!g)return;gesture.current=undefined;try{await flush(g);await request(cancel?'cancel':'end');}catch(e){onError(String(e));}};
  const grip=(label:string)=><button className="capture-dock-grip" aria-label={`Mover ${label}`} title="Arrastrá para mover · Flechas para ajustar · Doble clic para restablecer" onPointerDown={start} onPointerMove={move} onPointerUp={()=>void finish()} onPointerCancel={()=>void finish(true)} onLostPointerCapture={()=>void finish()} onDoubleClick={()=>void request('reset').catch(e=>onError(String(e)))} onKeyDown={e=>{const delta=({ArrowLeft:[-10,0],ArrowRight:[10,0],ArrowUp:[0,-10],ArrowDown:[0,10]} as Record<string,number[]>)[e.key];if(delta){e.preventDefault();void request('nudge',{x:delta[0],y:delta[1]}).catch(e=>onError(String(e)));}}}><GripVertical size={15}/></button>;
  const sizing=(label:string)=><span className="capture-dock-sizing" aria-label={`Tamaño de ${label}`}><button aria-label={`Reducir ${label}`} title={`Reducir (${Math.round(view.zoom*100)} %)`} disabled={view.zoom<=.75} onClick={()=>void change({zoom:Math.max(.75,Math.round((view.zoom-.1)*100)/100)})}><Minus size={13}/></button><button aria-label={`Ampliar ${label}`} title={`Ampliar (${Math.round(view.zoom*100)} %)`} disabled={view.zoom>=1.5} onClick={()=>void change({zoom:Math.min(1.5,Math.round((view.zoom+.1)*100)/100)})}><Plus size={13}/></button></span>;
  return {...view,change,grip,sizing};
}
