import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen,type UnlistenFn} from '@tauri-apps/api/event';
import {showWhenReady} from './screen-ready';
import './screen-recorder.css';

// A static, capture-excluded desktop underneath the independent ink and border.
// It consumes clicks outside the editor without activating the real app behind it.
export default function ScreenFrozen() {
  const [snapshot,setSnapshot]=useState<string>();
  useEffect(()=>{
    let alive=true,revision=0,url:string|undefined,currentGeneration:number|undefined;const remove:UnlistenFn[]=[];
    const reset=async(generation:number)=>{
      const request=++revision;
      const bytes=await invoke<ArrayBuffer>('screen_selection_image');
      if(!alive||request!==revision)return;
      if(!bytes.byteLength)throw Error('No se pudo preparar la pantalla congelada');
      const next=URL.createObjectURL(new Blob([bytes],{type:'image/bmp'}));
      try {
        const image=new Image();image.src=next;await image.decode();
        if(!alive||request!==revision){URL.revokeObjectURL(next);return;}
        if(url)URL.revokeObjectURL(url);url=next;setSnapshot(next);
        await showWhenReady('screen_frozen_ready',{generation},()=>alive&&request===revision);
      } catch(error){if(next!==url)URL.revokeObjectURL(next);throw error;}
    };
    const start=(generation:number)=>{if(generation===currentGeneration)return;currentGeneration=generation;const requested=revision+1;void reset(generation).catch(()=>{
      // Never leave an invisible blocker or an editor over a live desktop on failure.
      if(alive&&revision===requested)void invoke('screen_cancel_selection').catch(()=>{});
    });};
    void(async()=>{
      for(const promise of [
        listen<number>('screen-frozen-reset',e=>start(e.payload)),
        listen('screen-hide',()=>{++revision;currentGeneration=undefined;if(url)URL.revokeObjectURL(url);url=undefined;if(alive)setSnapshot(undefined);}),
      ]) {const off=await promise;if(alive)remove.push(off);else off();}
      const requested=revision;
      const generation=await invoke<number|null>('screen_frozen_state');
      if(alive&&requested===revision&&generation!==null&&generation!==undefined)start(generation);
    })().catch(()=>{});
    return()=>{alive=false;++revision;if(url)URL.revokeObjectURL(url);remove.forEach(off=>off());};
  },[]);
  return snapshot?<div className="screen-frozen-desktop" aria-label="Pantalla congelada para captura" style={{backgroundImage:`url("${snapshot}")`}} onPointerDown={e=>{e.preventDefault();e.stopPropagation();}} onContextMenu={e=>e.preventDefault()}/>:null;
}
