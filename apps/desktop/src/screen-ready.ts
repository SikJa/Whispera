import { invoke } from '@tauri-apps/api/core';
// Native windows stay hidden until both layout and their first transparent frame
// are ready. Reused windows keep the same document throughout a recording.
export async function showWhenReady(command:string,args:Record<string,unknown>|undefined,alive:()=>boolean) {
  await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
  for(let attempt=0;attempt<100&&alive();attempt++){
    if(await invoke<boolean>(command,args)!==false)return;
    await new Promise(resolve=>setTimeout(resolve,50));
  }
  if(alive())throw Error('La vista tardó demasiado en prepararse. Volvé a intentar.');
}
