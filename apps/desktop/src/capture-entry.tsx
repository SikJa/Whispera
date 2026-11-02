import {createRoot} from 'react-dom/client';
import {startHeartbeat} from './ui-heartbeat';
import '@fontsource-variable/instrument-sans';
import './style.css';
import './desktop.css';
import './plum-theme.css';

const view=new URLSearchParams(location.search).get('view');
document.documentElement.dataset.screenSelect='true';
if('__TAURI_INTERNALS__' in window && view!=='screen-freeze') {
  const stopHeartbeat=startHeartbeat();
  import.meta.hot?.dispose(stopHeartbeat);
}
// Prewarmed surfaces load their own implementation, not settings and every
// other editor. Each component still owns the existing native-ready handshake.
void (async()=>{
  let element:React.ReactNode=null;
  if(view==='screen-freeze'){
    const {default:ScreenFrozen}=await import('./ScreenFrozen');element=<ScreenFrozen/>;
  } else if(view==='screen-select'||view==='screen-indicator'){
    const {ScreenOverlay,ScreenIndicator}=await import('./ScreenCaptureViews');
    element=view==='screen-select'?<ScreenOverlay/>:<ScreenIndicator/>;
  } else if(view==='screen-ink'||view==='screen-tools'||view==='screen-hud'){
    const {ScreenInk,ScreenTools,ScreenHud}=await import('./ScreenEditor');
    element=view==='screen-ink'?<ScreenInk/>:view==='screen-tools'?<ScreenTools/>:<ScreenHud/>;
  }
  createRoot(document.getElementById('root')!).render(element);
})();
