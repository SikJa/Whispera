// Keep native background work running; suspend only hidden UI status requests.
export function visiblePoll(task:()=>Promise<void>,interval:number){
 let stopped=false,pending=false,again=false;
 let timer:ReturnType<typeof setTimeout>|undefined;
 const run=async()=>{
  clearTimeout(timer);
  if(stopped||document.hidden)return;
  if(pending){again=true;return;}
  pending=true;
  try{await task();}
  finally{
   pending=false;
   if(!stopped&&!document.hidden){if(again){again=false;void run();}else timer=setTimeout(run,interval);}
  }
 };
 const visibility=()=>{if(document.hidden){clearTimeout(timer);again=false;}else void run();};
 const focus=()=>void run();
 document.addEventListener('visibilitychange',visibility);window.addEventListener('focus',focus);void run();
 return()=>{stopped=true;clearTimeout(timer);document.removeEventListener('visibilitychange',visibility);window.removeEventListener('focus',focus);};
}
