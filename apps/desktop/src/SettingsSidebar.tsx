// Adapted from Yash Bavadiya's CollapsibleSidebar (MIT).
// License: docs/licenses/collapsible-sidebar.txt.
import {useEffect,useId,useRef,useState} from 'react';
import {createPortal} from 'react-dom';
import {LayoutGroup,motion,useReducedMotion} from 'motion/react';
import {ChevronDown} from 'lucide-react';
import './settings-sidebar.css';

type Item={id:string;name:string;icon:React.ComponentType<{size?:number}>;group?:string};
export default function SettingsSidebar({items,value,onChange,expanded,brand,update,footer,count}:{
  items:readonly Item[];value:string;onChange:(id:string)=>void;expanded:boolean;
  brand:React.ReactNode;update:React.ReactNode;footer:React.ReactNode;count:number;
}){
  const reduce=useReducedMotion(),group=useId();
  const [spaceCollapsed,setSpaceCollapsed]=useState(()=>{try{return localStorage.getItem('whispera.sidebar.space-collapsed')==='true';}catch{return false;}});
  useEffect(()=>{try{localStorage.setItem('whispera.sidebar.space-collapsed',String(spaceCollapsed));}catch{}},[spaceCollapsed]);
  const sections:{name:string;items:Item[]}[]=[];
  for(const item of items){if(item.group||!sections.length)sections.push({name:item.group??'',items:[]});sections[sections.length-1].items.push(item);}
  const spaceId=useId();
  const [tip,setTip]=useState<{text:string;left:number;top:number}|null>(null);
  const timer=useRef<ReturnType<typeof setTimeout>|undefined>(undefined);
  const hide=()=>{clearTimeout(timer.current);setTip(null);};
  useEffect(()=>{hide();return()=>clearTimeout(timer.current);},[expanded]);
  function show(element:HTMLButtonElement,text:string,keyboard=false){
    if(expanded)return;
    clearTimeout(timer.current);
    const rect=element.getBoundingClientRect();
    const next={text,left:rect.right+10,top:Math.max(8,Math.min(window.innerHeight-40,rect.top+(rect.height-32)/2))};
    if(keyboard||tip)setTip(next);else timer.current=setTimeout(()=>setTip(next),400);
  }
  return <LayoutGroup id={group}>
    <motion.aside className="desktop-sidebar settings-sidebar" data-expanded={expanded} initial={false}
      animate={{width:expanded?224:64}} transition={reduce?{duration:0}:{type:'spring',visualDuration:.3,bounce:0}} onPointerLeave={hide}>
      {brand}{update}
      <nav aria-label="Configuración" onScroll={hide}>
        {sections.map(section=><div className="sidebar-section" key={section.name}>
          {section.name==='Tu espacio'?<button className="nav-group-toggle" aria-label="Tu espacio" aria-expanded={!spaceCollapsed} aria-controls={spaceId} onClick={()=>{hide();setSpaceCollapsed(value=>!value);}}><span>Tu espacio</span><ChevronDown size={13}/></button>:section.name&&<p className="nav-group">{section.name}</p>}
          <div id={section.name==='Tu espacio'?spaceId:undefined} hidden={section.name==='Tu espacio'&&expanded&&spaceCollapsed}>
          {section.items.map(item=><div className="sidebar-item-group" key={item.id}>
          <button aria-label={item.name} aria-current={value===item.id?'page':undefined}
            className={value===item.id?'active':''} onClick={()=>{hide();onChange(item.id);}}
            onPointerEnter={e=>{if(e.pointerType!=='touch')show(e.currentTarget,item.name);}}
            onFocus={e=>{if(e.currentTarget.matches(':focus-visible'))show(e.currentTarget,item.name,true);}} onBlur={hide}>
            {value===item.id&&<motion.i className="sidebar-active-surface" aria-hidden="true" layoutId="active" layoutDependency={value}
              transition={reduce?{duration:0}:{type:'spring',visualDuration:.25,bounce:0}}/>}
            <item.icon size={17}/><span className="sidebar-label">{item.name}</span>
            {item.id==='dictionary'&&<small className="sidebar-count">{count}</small>}
          </button>
        </div>)}
          </div>
        </div>)}
      </nav>{footer}
    </motion.aside>
    {tip&&!expanded&&createPortal(<span className="sidebar-tooltip" aria-hidden="true" style={{left:tip.left,top:tip.top}}>{tip.text}</span>,document.body)}
  </LayoutGroup>;
}
