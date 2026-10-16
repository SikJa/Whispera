import {createRoot} from 'react-dom/client';
import {invoke} from '@tauri-apps/api/core';
import '@fontsource-variable/instrument-sans';
import {ScreenOverlay, ScreenIndicator} from './ScreenRecorder';
import {ScreenInk, ScreenTools, ScreenHud} from './ScreenEditor';
import ScreenFrozen from './ScreenFrozen';
import './style.css';
import './desktop.css';
import './plum-theme.css';

const view=new URLSearchParams(location.search).get('view');
document.documentElement.dataset.screenSelect='true';
if('__TAURI_INTERNALS__' in window && view!=='screen-freeze') {
  const heartbeat=()=>void invoke('ui_heartbeat').catch(()=>{});
  heartbeat();setInterval(heartbeat,2000);
}
const views:Record<string,React.ReactNode>={
  'screen-select':<ScreenOverlay/>, 'screen-freeze':<ScreenFrozen/>,
  'screen-ink':<ScreenInk/>, 'screen-tools':<ScreenTools/>,
  'screen-hud':<ScreenHud/>, 'screen-indicator':<ScreenIndicator/>,
};
createRoot(document.getElementById('root')!).render(views[view??'']??null);
