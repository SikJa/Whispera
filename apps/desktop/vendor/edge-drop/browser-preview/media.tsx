import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Check, X, FolderOpen, Plus, Layers, ArrowUpToLine, ArrowDownToLine, Trash2, Download, AlignCenter, Paintbrush, PanelLeft, Image as ImageIcon, Droplets } from 'lucide-react';
import { arrange, loadImage, renderComposition, renderBackground, type Capture, type Layer, type Composition } from './composition';
import { backgrounds, gradientBitmap } from './backgrounds';
import { EditorRange, EditorToggle, RatioSelect } from './EditorControls';
import './media.css';
import { presets } from '../../../src/palette';

const native = (window as any).__TAURI_INTERNALS__;
const invoke = (name:string,args:any={}) => native?.invoke(name,args) ?? Promise.resolve();
const params = new URLSearchParams(location.search);
const mode = 'canvas';
const id = params.get('id') || '';
const asset = (path:string) => native ? native.convertFileSrc(path,'asset') : path;
const filePaths = (data:any):string[] => data.kind === 'image' ? [data.imageId] : data.kind === 'image-collection' ? data.images.map((i:any) => i.imageId) : data.paths || [];
const imagePaths = (data:any) => filePaths(data).filter(p => /\.(png|jpg|jpeg|webp)$/i.test(p));
const gradientSwatches=backgrounds.map(p=>({...p,src:gradientBitmap(p.id,72,56).toDataURL()}));
type ButtonProps = { title:string; children:React.ReactNode; onClick:()=>void; disabled?:boolean; active?:boolean };
function Tool({title,children,onClick,disabled,active}:ButtonProps) { return <button className={`media-tool${active?' active':''}`} title={title} aria-label={title} disabled={disabled} onClick={onClick}>{children}</button>; }

function MediaApp() {
  const [items,setItems] = useState<any[]>([]);
  const [error,setError] = useState('');
  const [copied,setCopied] = useState(false);
  const [layers,setLayers] = useState<Layer[]>([]);
  const [selected,setSelected] = useState<string>();
  const [picker,setPicker] = useState(false);
  const [backgroundPicker,setBackgroundPicker] = useState(false);
  const [panel,setPanel] = useState(true);
  const [autoBalance,setAutoBalance] = useState(true);
  const [backgroundPreview,setBackgroundPreview] = useState('');
  const [busy,setBusy] = useState(false);
  const [ready,setReady] = useState(false);
  const [stageSize,setStageSize] = useState({width:700,height:437.5});
  const [options,setOptions] = useState<Composition>({width:960,height:600,background:'gradient:iris',padding:64,radius:18,shadow:30,blur:32});
  const stage = useRef<HTMLDivElement>(null);
  useEffect(()=>{
    let active=true;
    void renderBackground(options,layers[0]?.src).then(canvas=>{if(active)setBackgroundPreview(canvas.toDataURL());}).catch(e=>{if(active)setError(String(e));});
    return()=>{active=false;};
  },[options.background,options.backgroundSource,options.width,options.height,options.blur,layers[0]?.src]);
  const refresh = async () => { const data = native ? await invoke('library_state',{revision:null}) : {items:(window as any).__MEDIA_FIXTURES__ || JSON.parse(sessionStorage.getItem('whispera-media-preview') || '[]')}; setItems(data.items || []); return data.items || []; };
  const run = async (action:()=>Promise<any>) => { setError(''); try { return await action(); } catch(e) {setError(String(e));} };
  const close = () => void run(async() => { document.body.classList.add('closing'); await new Promise(r => setTimeout(r,150)); if(native)await invoke('library_media_window',{action:'close'}); });
  const captures = async(paths:string[]):Promise<Capture[]> => {
    const result:Capture[]=[];
    for(const path of paths.slice(0,10)) { const image = await loadImage(asset(path)); result.push({key:crypto.randomUUID(),path,src:asset(path),width:image.naturalWidth,height:image.naturalHeight}); }
    return result;
  };
  const add = async(paths:string[]) => { const images = await captures(paths); setLayers(previous => arrange([...previous,...images].slice(0,10),options)); setPicker(false); };
  useEffect(() => {
    void run(async() => {
      const list = await refresh(); const item = list.find((i:any) => i.id === id) || list[0];
      if(!item)throw new Error('No hay capturas disponibles');
      if(mode === 'canvas') setLayers(arrange(await captures(imagePaths(item.data)),options));
      setReady(true);
    }).finally(()=>setReady(true));
    const key = (e:KeyboardEvent) => {if(e.key === 'Escape')close();}; document.addEventListener('keydown',key);
    return () => document.removeEventListener('keydown',key);
  },[]);
  useEffect(() => {
    if(!native || !ready)return;
    const panel = document.querySelector('.media-surface')!;
    let shown=false,last='';
    const update = () => { const r = panel.getBoundingClientRect();const rects=[{x:r.x,y:r.y,width:r.width,height:r.height}];const signature=JSON.stringify(rects);if(signature===last)return;last=signature;void invoke('recorder_region',{rects}).then(() => {if(!shown){shown=true;return invoke('library_media_window',{action:'show'});}}); };
    const observer=new ResizeObserver(update);observer.observe(panel);update();
    // ResizeObserver does not track CSS transforms during the reveal animation.
    const frames=setInterval(update,50);const end=setTimeout(()=>clearInterval(frames),550);
    return()=>{observer.disconnect();clearInterval(frames);clearTimeout(end);};
  },[ready]);
  useEffect(() => {
    if(mode !== 'canvas')return;
    const workspace=document.querySelector('.canvas-workspace')!;
    const resize=()=>{const style=getComputedStyle(workspace);const width=workspace.clientWidth-parseFloat(style.paddingLeft)-parseFloat(style.paddingRight);const height=workspace.clientHeight-parseFloat(style.paddingTop)-parseFloat(style.paddingBottom)-36;const ratio=options.width/options.height;const w=Math.min(width,height*ratio);setStageSize({width:w,height:w/ratio});};
    const observer=new ResizeObserver(resize);observer.observe(workspace);resize();return()=>observer.disconnect();
  },[options.width,options.height]);
  useEffect(() => {
    if(!native || mode !== 'canvas')return;
    let unlisten:number|undefined;
    const label=native.metadata?.currentWindow?.label;
    const handler=native.transformCallback(({payload}:any) => { const paths=payload.paths?.filter((p:string) => /\.png$/i.test(p)) || []; if(paths.length)void run(async() => {await invoke('library_action',{action:'add',id:null,value:paths});await refresh();await add(paths);}); });
    void invoke('plugin:event|listen',{event:'tauri://drag-drop',target:label?{kind:'WebviewWindow',label}:{kind:'Any'},handler}).then((eventId:number) => unlisten=eventId);
    return()=>{if(unlisten != null)void invoke('plugin:event|unlisten',{event:'tauri://drag-drop',eventId:unlisten});};
  },[options]);
  function patch(change:Partial<Composition>,balance=false) { const next={...options,...change};setOptions(next);if(balance&&autoBalance)setLayers(arrange(layers,next)); }
  function pointer(e:React.PointerEvent<HTMLElement>,layer:Layer,resize=false) {
    if(e.button!==0)return;e.stopPropagation();e.currentTarget.setPointerCapture(e.pointerId);setSelected(layer.key);
    const rect=stage.current!.getBoundingClientRect();const x=e.clientX,y=e.clientY;
    const move=(event:PointerEvent)=>{ const dx=(event.clientX-x)*options.width/rect.width;const dy=(event.clientY-y)*options.height/rect.height;
      setLayers(previous=>previous.map(item=>item.key!==layer.key?item:resize?{...item,w:Math.max(32,Math.min(options.width,layer.w+dx)),h:Math.max(32,Math.min(options.width,layer.w+dx))*layer.height/layer.width}:{...item,x:Math.max(0,Math.min(options.width-layer.w,layer.x+dx)),y:Math.max(0,Math.min(options.height-layer.h,layer.y+dy))})); };
    const element=e.currentTarget;element.addEventListener('pointermove',move);element.addEventListener('pointerup',()=>element.removeEventListener('pointermove',move),{once:true});element.addEventListener('pointercancel',()=>element.removeEventListener('pointermove',move),{once:true});
  }
  const exportPng = async() => {setBusy(true);await run(async() => {const canvas=await renderComposition(layers,options); const png=canvas.toDataURL('image/png'); if(native)await invoke('library_media_export',{png});else {const a=document.createElement('a');a.href=png;a.download='Whispera.png';a.click();}setCopied(true);setTimeout(()=>setCopied(false),1600);});setBusy(false);};
  const moveLayer = (front:boolean) => setLayers(previous=>{const layer=previous.find(l=>l.key===selected);if(!layer)return previous;const rest=previous.filter(l=>l!==layer);return front?[...rest,layer]:[layer,...rest];});
  const imageChoices=items.flatMap(item=>imagePaths(item.data).map(path=>({path,id:item.id})));
  return <main className={`media-surface canvas ${ready?'ready':''}`}>
    <>
      <header className="canvas-header" onPointerDown={e=>{if(!(e.target as Element).closest('button')&&native)void invoke('library_media_window',{action:'drag'});}}><span><Tool title={panel?'Ocultar ajustes':'Mostrar ajustes'} active={panel} onClick={()=>setPanel(!panel)}><PanelLeft size={16}/></Tool><Layers size={16}/> Editor de imagen</span><div><button className="export-btn" disabled={busy||!layers.length} onClick={()=>void exportPng()}>{copied?<Check size={14}/>:<Download size={14}/>} {busy?'Exportando...':copied?'Copiado':'Guardar y copiar'}</button><Tool title="Cerrar" onClick={close}><X size={16}/></Tool></div></header>
      <div className={`canvas-layout${panel?'':' panel-collapsed'}`}>
        <aside className="canvas-settings">
          <h2>Degradados</h2><div className="gradient-swatches">{gradientSwatches.map(p=><button key={p.id} title={p.name} aria-label={`Fondo ${p.name}`} aria-pressed={options.background===`gradient:${p.id}`} onClick={()=>patch({background:`gradient:${p.id}`})}><img src={p.src} alt=""/>{options.background===`gradient:${p.id}`&&<Check size={13}/>}</button>)}</div>
          <h2>Imagen de fondo</h2><div className="background-modes"><button aria-pressed={options.background==='blur'} disabled={!layers.length} onClick={()=>patch({background:'blur'})}><Droplets size={14}/>Difuminado</button><button aria-pressed={options.background==='wallpaper'} onClick={()=>{setBackgroundPicker(!backgroundPicker);void refresh();}}><ImageIcon size={14}/>Elegir imagen</button></div>
          {backgroundPicker&&<div className="capture-picker">{imageChoices.map((choice,index)=><button key={`${choice.path}-${index}`} title="Usar como fondo" onClick={()=>{patch({background:'wallpaper',backgroundSource:asset(choice.path)});setBackgroundPicker(false);}}><img src={asset(choice.path)} alt={`Fondo ${index+1}`}/></button>)}</div>}
          {options.background==='blur'&&<EditorRange label="Desenfoque" min={4} max={64} value={options.blur??32} unit="px" onChange={blur=>patch({blur})}/>}
          <h2>Color</h2><div className="background-swatches">{[{name:'Transparente',hex:'transparent'},...presets].map(preset=><button key={preset.hex} title={preset.name} aria-label={preset.name} aria-pressed={options.background===preset.hex} style={{background:preset.hex}} className={preset.hex==='transparent'?'checker':''} onClick={()=>patch({background:preset.hex})}/>)}<label className="custom-color" title="Color personalizado"><Paintbrush size={16}/><input type="color" aria-label="Color personalizado" value={/^#[0-9a-f]{6}$/i.test(options.background)?options.background:'#151517'} onChange={e=>patch({background:e.target.value})}/></label></div>
          <RatioSelect value={options.width/options.height} onChange={ratio=>patch({height:Math.round(options.width/ratio)},true)}/>
          {[['Margen','padding',0,160],['Esquinas','radius',0,96],['Sombra','shadow',0,70]].map(([name,key,min,max])=><EditorRange key={String(key)} label={String(name)} min={Number(min)} max={Number(max)} value={Number(options[key as keyof Composition])} unit={key==='shadow'?'%':'px'} onChange={value=>patch({[key]:value},key==='padding')}/>)}
          <EditorToggle checked={autoBalance} onChange={checked=>{setAutoBalance(checked);if(checked)setLayers(arrange(layers,options));}}/>
          <button className="row-command" onClick={()=>setLayers(arrange(layers,options))}><AlignCenter size={15}/> Equilibrar</button>
          <h2>Capturas</h2><button className="row-command" onClick={()=>{setPicker(!picker);void refresh();}}><Plus size={15}/> Agregar captura</button>
          {picker&&<div className="capture-picker">{imageChoices.map((choice,index)=><button key={`${choice.path}-${index}`} title="Agregar al lienzo" onClick={()=>void run(()=>add([choice.path]))}><img src={asset(choice.path)} alt={`Captura ${index+1}`}/></button>)}<button className="row-command" onClick={()=>void run(async()=>{if(native){await invoke('library_pick');await refresh();}})}><FolderOpen size={15}/> Importar</button></div>}
          <div className="layer-list">{layers.map((layer,index)=><button key={layer.key} aria-pressed={selected===layer.key} onClick={()=>setSelected(layer.key)}><img src={layer.src} alt=""/><span>Captura {index+1}</span></button>)}</div>
          <div className="layer-actions"><Tool title="Traer adelante" disabled={!selected} onClick={()=>moveLayer(true)}><ArrowUpToLine size={16}/></Tool><Tool title="Enviar atras" disabled={!selected} onClick={()=>moveLayer(false)}><ArrowDownToLine size={16}/></Tool><Tool title="Quitar captura" disabled={!selected} onClick={()=>setLayers(layers.filter(l=>l.key!==selected))}><Trash2 size={16}/></Tool></div>
        </aside>
        <section className="canvas-workspace" onDragOver={e=>e.preventDefault()} onDrop={e=>{e.preventDefault();void run(async()=>{const req=JSON.parse(e.dataTransfer.getData('text/x-edge-preview')||'{}');const item=items.find(i=>i.id===req.id);if(item)await add(imagePaths(item.data));});}}>
          <div className={`composition-stage ${options.background==='transparent'?'checker':''}`} ref={stage} style={{width:stageSize.width,height:stageSize.height}} onPointerDown={()=>setSelected(undefined)}>
            {backgroundPreview&&<img className="composition-background" src={backgroundPreview} alt="" draggable={false}/>}
            {layers.map(layer=><div key={layer.key} className={`composition-layer ${selected===layer.key?'selected':''}`} style={{left:`${layer.x/options.width*100}%`,top:`${layer.y/options.height*100}%`,width:`${layer.w/options.width*100}%`,height:`${layer.h/options.height*100}%`}} onPointerDown={e=>pointer(e,layer)}><img src={layer.src} alt="Captura" draggable={false} style={{borderRadius:`${options.radius/options.width*(stage.current?.clientWidth||700)}px`,boxShadow:`0 ${12/options.width*(stage.current?.clientWidth||700)}px ${30/options.width*(stage.current?.clientWidth||700)}px rgba(0,0,0,${options.shadow/100})`}}/>{selected===layer.key&&<button className="resize-handle" title="Redimensionar captura" aria-label="Redimensionar captura" onPointerDown={e=>pointer(e,layer,true)}/>}</div>)}
          </div>
          <footer>{layers.length} {layers.length===1?'captura':'capturas'}<span>{options.width} x {options.height}</span></footer>
        </section>
      </div>
    </>
    {error&&<div className="media-error" role="alert">{error}</div>}
  </main>;
}
createRoot(document.getElementById('root')!).render(<MediaApp/>);
