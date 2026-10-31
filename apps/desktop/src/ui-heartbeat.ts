/// <reference types="vite/client" />
import {invoke} from '@tauri-apps/api/core';

// The native watchdog excludes hidden windows and grants a fresh grace period
// when they reappear. Keep its heartbeat active without waking hidden views.
export function startHeartbeat() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stopped=false;
  const tick=()=>{
    clearTimeout(timer);
    if(stopped || document.hidden)return;
    void invoke('ui_heartbeat').catch(()=>{});
    timer=setTimeout(tick,2000);
  };
  document.addEventListener('visibilitychange',tick);
  tick();
  return ()=>{stopped=true;clearTimeout(timer);document.removeEventListener('visibilitychange',tick);};
}
