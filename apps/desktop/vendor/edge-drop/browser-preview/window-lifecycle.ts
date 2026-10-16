type Rect = { x:number; y:number; width:number; height:number };
type Invoke = (command:string,args:Record<string,unknown>)=>Promise<unknown>;

// Visibility follows explicit panel state, never the presence of hidden DOM nodes.
export function libraryWindow(invoke:Invoke, rectangles:()=>Rect[]) {
  let wanted=false, shown=false, busy=false, revision=0, signature='';
  const close=async()=>{
    wanted=false;shown=false;signature='';++revision;
    await invoke('library_window',{operation:'hide'});
  };
  const sync=async()=>{
    if(!wanted||busy)return;
    const rects=rectangles();
    if(!rects.length){if(shown)await close();return;}
    const next=JSON.stringify(rects);
    if(shown&&signature===next)return;
    const current=revision;
    busy=true;
    try{
      await invoke('recorder_region',{rects});
      if(!wanted||current!==revision)return;
      if(!shown){
        await invoke('library_window',{operation:'show'});
        // A close can arrive while the native show command is being dispatched.
        if(!wanted||current!==revision){await invoke('library_window',{operation:'hide'});return;}
        shown=true;
      }
      signature=next;
    }catch(error){
      if(current===revision){await close();}
      throw error;
    }finally{busy=false;}
  };
  return {close,sync,open:()=>{wanted=true;signature='';++revision;return sync();}};
}
