import { invoke } from '@tauri-apps/api/core';
// Hidden WebViews can suspend animation frames. Layout is already committed by
// the caller; bound the paint wait so a hidden window can become visible.
function nextFrame() {
  return new Promise<void>(resolve => {
    const timer=setTimeout(()=>{cancelAnimationFrame(frame);resolve();},40);
    const frame=requestAnimationFrame(()=>{clearTimeout(timer);resolve();});
  });
}
export async function showWhenReady(command:string,args:Record<string,unknown>|undefined,alive:()=>boolean) {
  await nextFrame();if(!alive())return;await nextFrame();
  for(let attempt=0;attempt<100&&alive();attempt++){
    if(await invoke<boolean>(command,args)!==false)return;
    await new Promise(resolve=>setTimeout(resolve,50));
  }
  if(alive())throw Error('La vista tardó demasiado en prepararse. Volvé a intentar.');
}
