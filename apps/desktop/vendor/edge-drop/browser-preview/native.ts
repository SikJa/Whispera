import { DEFAULT_SETTINGS } from '../shared/types';
import { libraryWindow } from './window-lifecycle';
const invoke=(command:string,args:any={})=>(window as any).__TAURI_INTERNALS__.invoke(command,args);
const listeners=new Map<string,Set<Function>>();
const emit=(name:string,...args:any[])=>listeners.get(name)?.forEach(f=>f(...args));
const subscribe=(name:string,fn:Function)=>{if(!listeners.has(name))listeners.set(name,new Set());listeners.get(name)!.add(fn);return()=>listeners.get(name)!.delete(fn);};
const toast=(message:string)=>emit('Toast',{id:crypto.randomUUID(),message,tone:'info'});
let state:any={items:[],settings:{...DEFAULT_SETTINGS,launchAtLogin:false,stickPosition:'left',captureGlobal:true,hoverActivation:false,language:'es',tutorialCompleted:true,panelHeight:.9,autoDeleteHours:48,autoUpdates:false,updateMode:'off'},version:'Whispera'};
let busy=false;
let holdsSettings=()=>false;
let refreshQueue:Promise<any>=Promise.resolve();
const refresh=()=>{const next=refreshQueue.then(async()=>{const data=await invoke('library_state',{revision:state.revision||null});if(data.unchanged)return state;for(const item of data.items){const d=item.data;if(d.kind==='image')d.preview=(window as any).__TAURI_INTERNALS__.convertFileSrc(d.imageId,'asset');if(d.kind==='image-collection')for(const image of d.images)image.preview=(window as any).__TAURI_INTERNALS__.convertFileSrc(image.imageId,'asset');}state={...data,settings:{...state.settings,...(data.settings||{})}};emit('Items',data.items);if(!holdsSettings())emit('Settings',state.settings);return state;});refreshQueue=next.catch(()=>{});return next;};
const act=async(action:string,id:string|null=null,value:any=null)=>{const result=await invoke('library_action',{action,id,value});await refresh();return result;};
const reqAction=async(action:string,req:any)=>act(action,req.id,req);
const api:any={loadState:refresh,getUpdateState:async()=>null,setPinned:(id:string,pinned:boolean)=>act('pin',id,pinned).then(()=>state.items),deleteItem:(id:string)=>act('delete',id).then(()=>state.items),deleteBatchItems:async(ids:string[])=>{for(const id of ids)await act('delete',id);return state.items;},clearItems:()=>act('clear').then(()=>state.items),getFullText:async(id:string)=>state.items.find((i:any)=>i.id===id)?.data.text||'',copyItem:(id:string)=>act('copy',id),pasteItem:(id:string)=>act('paste',id),copySubitem:(req:any)=>reqAction('copy',req),pasteSubitem:(req:any)=>reqAction('paste',req),pasteEmoji:async()=>false,removeSubitem:async()=>{toast('Separá el elemento antes de eliminarlo.');return false;},splitItem:(req:any)=>act('split',req.id,req).then(()=>true),mergeItems:async(a:string,b:string)=>{try{await act('merge',a,b);return{ok:true};}catch(e){toast(String(e));return{ok:false,reason:'incompatible'};}},addFiles:(paths:string[])=>act('add',null,paths).then(()=>state.items),addItemData:async()=>{toast('Usá el portapapeles o el selector nativo.');return state.items;},updateSettings:async(patch:any)=>{const settings=await act('settings',null,patch);return{...state.settings,...settings};},refreshLaunchAtLogin:async()=>state.settings,getDisplays:async()=>[{id:1,bounds:{x:0,y:0,width:innerWidth,height:innerHeight},isPrimary:true,isCurrent:true,label:'Monitor actual'}],setInteractive:async()=>{},setPreviewMode:async()=>{},pauseHotkey:async()=>{},focusWindow:async()=>{},setInternalDrag:()=>{},broadcastTutorialStep:()=>{},prestageDrag:()=>{},startDrag:(req:any)=>{if(req.imageId||req.paths){toast('Separá primero el subelemento para arrastrarlo.');return;}void invoke('library_drag',{id:req.id}).catch(e=>toast(String(e))).finally(()=>emit('DragEnd'));},revealFile:async()=>{toast('Abrir ubicación todavía no conectado.');return false;},minimizeWindow:()=>invoke('library_toggle'),quitApp:()=>invoke('library_toggle'),checkForUpdatesManual:async()=>({status:'error',error:'El actualizador de Whispera todavía no está configurado.'}),installUpdate:async()=>toast('Actualizador de Whispera pendiente'),startUpdateDownload:async()=>toast('Actualizador de Whispera pendiente')};
for(const name of ['Items','Settings','Toggle','OpenSettings','DragEnd','InternalDrop','CursorEdge','Toast','CopyFlare','TutorialStep','UpdateAvailable','UpdateProgress','UpdateDownloaded'])api['on'+name]=(fn:Function)=>subscribe(name,fn);
window.edge=api;
(window as any).__previewFiles=new Proxy({}, {get:(_,path)=>(window as any).__TAURI_INTERNALS__.convertFileSrc(String(path),'asset')});
document.getElementById('preview-controls')?.remove();
await refresh();
// Hydration reuses this snapshot; opening the shelf must not rewrite preferences.
api.loadState=async()=>state;
await import('../src/main');
const {useStore}=await import('../src/store/appStore');
holdsSettings=()=>Boolean(useStore.getState().sliderActive||useStore.getState().edgeTransition?.active);
const surface=libraryWindow(invoke,()=>Array.from(document.querySelectorAll('.blade,.preview-flyout,.palette-popover,.split-dropzone'))
  .filter(el=>getComputedStyle(el).visibility!=='hidden'&&getComputedStyle(el).display!=='none')
  .map(el=>el.getBoundingClientRect())
  .map(r=>({x:Math.max(0,r.x),y:Math.max(0,r.y),width:Math.min(innerWidth,r.right)-Math.max(0,r.x),height:Math.min(innerHeight,r.bottom)-Math.max(0,r.y)}))
  .filter(r=>r.width>0&&r.height>0));
const closePanel=()=>{
  useStore.setState({open:false,previewItemId:null,previewItemRect:null});
  return surface.close();
};
const surfaceError=(error:unknown)=>{void closePanel().catch(()=>{});toast(String(error));};
useStore.subscribe((next,previous)=>{
  if(previous.open&&!next.open)void surface.close().catch(()=>{});
  if(!previous.open&&next.open)void surface.open().catch(surfaceError);
});
const nativeListen=(event:string,callback:Function)=>invoke('plugin:event|listen',{event,target:{kind:'WebviewWindow',label:'library'},handler:(window as any).__TAURI_INTERNALS__.transformCallback(callback)});
await nativeListen('library-open',()=>{
  void refresh().catch(()=>{});
  window.dispatchEvent(new Event('panel:open-request'));
  if(useStore.getState().open)void surface.open().catch(surfaceError);
  else useStore.getState().setOpen(true);
});
await nativeListen('library-close',()=>void closePanel().catch(()=>{}));
await nativeListen('library-changed',()=>void refresh().catch(()=>{}));
await nativeListen('tauri://drag-drop',async({payload}:any)=>{
  const point=payload.position;
  const element=document.elementFromPoint(point.x/devicePixelRatio,point.y/devicePixelRatio);
  const destination=element?.closest('[data-id]')?.getAttribute('data-id');
  const request=useStore.getState().internalDragReq;
  try {
    if(request&&destination&&destination!==request.id) await act('merge',request.id,destination);
    else if(request&&(request.imageId||request.paths)&&element?.closest('.split-dropzone')) await act('split',request.id,request);
    else if(!request) await act('add',null,payload.paths);
  } catch(e){toast(String(e));}
  useStore.getState().setInternalDragReq(null);emit('DragEnd');
});
const ready=setInterval(()=>{if(useStore.getState().hydrated){useStore.getState().setOpen(true);clearInterval(ready);}},50);
setInterval(async()=>{if(busy)return;busy=true;try{await refresh();}catch(e){toast(String(e));}finally{busy=false;}},1500);
let cursorBusy=false;
setInterval(async()=>{if(cursorBusy)return;cursorBusy=true;try{const point=await invoke('library_cursor');if(point&&typeof point.x==='number')emit('CursorEdge',point);}catch{}finally{cursorBusy=false;}},100);
setInterval(()=>{if(useStore.getState().open)void surface.sync().catch(surfaceError);},100);
let nativeDragInFlight=false;
api.startDrag=(req:any)=>{
  if(nativeDragInFlight)return;
  nativeDragInFlight=true;
  useStore.setState({nativeDragActive:true,internalDragReq:req});
  void invoke('library_drag',{id:req.id,request:req}).catch(e=>toast(String(e))).finally(()=>{
    nativeDragInFlight=false;
    useStore.setState({nativeDragActive:false});
    useStore.getState().setInternalDragReq(null);
    emit('DragEnd');
  });
};
// A cancelled DOM drag must not release the longer-lived native OLE operation.
document.addEventListener('dragend',e=>{if(nativeDragInFlight)e.stopImmediatePropagation();},true);
api.removeSubitem=(req:any)=>act('remove-part',req.id,req).then(()=>true);
api.focusWindow=(focus:boolean)=>focus?invoke('library_window',{operation:'focus'}):Promise.resolve();
api.minimizeWindow=closePanel;
api.quitApp=api.minimizeWindow;
api.openWhisperaUpdates=()=>invoke('open_updates');
api.revealFile=(path:string)=>{const item=state.items.find((i:any)=>i.data.paths?.includes(path)||i.data.imageId===path);return item?act('reveal',item.id,item.data.kind==='files'?{paths:[path]}:null):Promise.reject('Archivo inexistente');};
const staged=new Set<string>();
let staging=false;
api.prestageDrag=(req:any)=>{
  const key=JSON.stringify(req);
  if(staging||staged.has(key)||nativeDragInFlight)return;
  staging=true;
  void invoke('library_prepare_drag',{id:req.id,request:req}).then(()=>{
    staged.add(key);
    if(staged.size>32)staged.delete(staged.values().next().value!);
  }).catch(()=>{}).finally(()=>{staging=false;});
};
document.addEventListener('keydown',e=>{if(e.key==='Escape')void closePanel().catch(()=>{});});
