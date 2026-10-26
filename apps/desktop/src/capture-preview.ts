import {installNativeMock} from '../tests/native-mock.mjs';
import './capture-preview.css';

const query=new URLSearchParams(location.search);
const view=query.get('view');
const kind=query.get('kind')==='image'?'image':'video';
if(view){
  installNativeMock({kind,selectionPhase:kind==='image'?'editing':'recording'});
  const win=window as unknown as {__TAURI_INTERNALS__:{invoke:(command:string,args?:Record<string,unknown>)=>Promise<any>}};
  const original=win.__TAURI_INTERNALS__.invoke;
  let size={width:50,height:540},zoom=1,horizontal=false;
  const post=(type:string,args:Record<string,unknown>)=>parent.postMessage({type,view,...args},location.origin);
  win.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
    const result=await original(command,args);
    if(command==='screen_overlay_customize'){
      if(args.action==='size')size={width:Number(args.width),height:Number(args.height)};
      if(args.action==='configure'){zoom=result.zoom;horizontal=!!result.horizontal;}
      if(['size','configure','reset'].includes(String(args.action)))post('size',{width:Math.ceil(size.width*zoom),height:Math.ceil(size.height*zoom),horizontal,reset:args.action==='reset'});
      if(['begin','move','end','cancel'].includes(String(args.action)))post(String(args.action),args);
    }
    if(command==='screen_tools_panel'){
      post('panel',{open:args.open,width:Math.ceil(size.width*zoom),height:Math.ceil(size.height*zoom),panelHeight:Number(args.panelHeight),horizontal});
      return {railX:4,railY:4,menuX:horizontal?4:4+size.width*zoom,menuY:horizontal?size.height*zoom+10:4};
    }
    return result;
  };
  await import('./capture-entry');
}else{
  document.body.className='capture-preview-page';
  document.getElementById('root')!.innerHTML=`<header><img src="/cristal/32x32.png" alt=""><strong>Whispera</strong><span>Vista previa · no graba</span><nav aria-label="Modo de captura"><a href="?kind=image" ${kind==='image'?'aria-current="page"':''}>Imagen</a><a href="?kind=video" ${kind==='video'?'aria-current="page"':''}>Video</a></nav></header><main aria-label="Pantalla completa"><div class="preview-workspace"><img src="/cristal/256x256.png" alt="Whispera"><h1>Pantalla completa</h1><p>1920 × 1080</p></div><iframe title="Herramientas" data-view="screen-tools" src="?view=screen-tools&kind=${kind}"></iframe><iframe title="Acciones" data-view="screen-hud" src="?view=screen-hud&kind=${kind}"></iframe></main>`;
  const stage=document.querySelector('main')!;
  const states=new Map<string,{width:number;height:number;horizontal:boolean;pinned?:{x:number;y:number};origin?:{x:number;y:number}}>();
  const place=(name:string)=>{
    const frame=document.querySelector<HTMLIFrameElement>(`iframe[data-view="${name}"]`)!;
    const s=states.get(name)!;
    frame.style.width=s.width+'px';frame.style.height=s.height+'px';
    const x=s.pinned?.x??(name==='screen-tools'&&!s.horizontal?12:(stage.clientWidth-s.width)/2);
    const y=s.pinned?.y??(name==='screen-tools'?(s.horizontal?12:(stage.clientHeight-s.height)/2):stage.clientHeight-s.height-12);
    frame.style.left=Math.max(4,Math.min(stage.clientWidth-s.width-4,x))+'px';
    frame.style.top=Math.max(4,Math.min(stage.clientHeight-s.height-4,y))+'px';
  };
  window.addEventListener('message',e=>{
    if(e.origin!==location.origin)return;
    const frame=[...document.querySelectorAll('iframe')].find(f=>f.contentWindow===e.source);
    if(!frame)return;
    const name=frame.dataset.view!,data=e.data;
    const s=states.get(name)??{width:50,height:540,horizontal:false};states.set(name,s);
    if(data.type==='size'){s.width=data.width;s.height=data.height;s.horizontal=data.horizontal;if(data.reset)s.pinned=undefined;}
    if(data.type==='begin')s.origin={x:frame.offsetLeft,y:frame.offsetTop};
    if(data.type==='move'&&s.origin)s.pinned={x:s.origin.x+data.x,y:s.origin.y+data.y};
    if(data.type==='cancel')s.pinned=s.origin;
    if(data.type==='panel'){s.width=data.horizontal?Math.max(data.width,data.open?300:0):data.width+(data.open?300:0);s.height=data.horizontal?data.height+(data.open?data.panelHeight+18:0):Math.max(data.height,data.open?data.panelHeight+8:0);}
    place(name);
  });
  new ResizeObserver(()=>states.forEach((_,name)=>place(name))).observe(stage);
}
