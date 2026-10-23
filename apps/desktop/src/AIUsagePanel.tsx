import {useEffect,useRef,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {ArrowUpRight,RefreshCw,Clock3,Link2} from 'lucide-react';
import {Button} from '../vendor/components/ui/button';
import {native} from './client';
import './ai-usage.css';

type Quota={label:string;usedPercent:number;resetAt:number|null;windowMinutes:number|null};
type Credit={label:string;balance:string|null;unlimited:boolean};
type Provider={id:string;name:string;status:string;message:string;observedAt:number|null;plan:string|null;windows:Quota[];credits:Credit[]};
type Snapshot={providers:Provider[];claudeConnection:{connected:boolean;settingsPath:string}};
const empty:Snapshot={providers:[{id:'codex',name:'OpenAI · Codex',status:'unavailable',message:'La consulta requiere Whispera en Windows y una sesión de Codex.',observedAt:null,plan:null,windows:[],credits:[]},{id:'claude',name:'Anthropic · Claude',status:'unavailable',message:'Conectá Claude Code para recibir tus cuotas.',observedAt:null,plan:null,windows:[],credits:[]}],claudeConnection:{connected:false,settingsPath:''}};
const percent=(value:number)=>new Intl.NumberFormat('es',{maximumFractionDigits:1}).format(value);
function duration(minutes:number|null){
  if(!minutes)return 'Ventana del proveedor';
  if(minutes%1440===0)return `${minutes/1440} ${minutes===1440?'día':'días'}`;
  if(minutes%60===0)return `${minutes/60} ${minutes===60?'hora':'horas'}`;
  return `${minutes} minutos`;
}
function countdown(reset:number|null,now:number){
  if(!reset)return 'Reinicio no informado';
  const seconds=Math.max(0,reset-now);
  if(!seconds)return 'Esperando actualización del reinicio';
  const minutes=Math.ceil(seconds/60),days=Math.floor(minutes/1440),hours=Math.floor(minutes%1440/60);
  return `Se renueva en ${days?`${days} d `:''}${hours?`${hours} h `:''}${minutes%60||(!days&&!hours)?`${minutes%60} min`:''}`.trim();
}
function QuotaRow({quota,now,previous}:{quota:Quota;now:number;previous:boolean}){
  const expired=quota.resetAt!==null&&quota.resetAt<=now;
  const remaining=Math.max(0,Math.min(100,100-quota.usedPercent));
  const unknown=expired;
  return <div className="ai-quota" data-previous={previous||expired}>
    <div className="ai-quota-heading"><div><h3>{quota.label}</h3><span>{duration(quota.windowMinutes)}</span></div>
      <div className="ai-quota-remaining"><strong>{unknown?'—':percent(remaining)}{!unknown&&<small>%</small>}</strong><span>{unknown?'por confirmar':previous?'último restante':'restante'}</span></div></div>
    <div className="ai-quota-track" role="progressbar" aria-label={`${quota.label}: cuota restante`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={unknown?undefined:remaining} aria-valuetext={unknown?'Esperando datos nuevos':`${percent(remaining)}% restante${previous?', dato anterior':''}`}><span style={{width:unknown?'0%':`${remaining}%`}}/></div>
    <div className="ai-quota-meta"><span>{unknown?'La ventana anterior terminó':`${percent(quota.usedPercent)}% utilizado`}</span><span title={quota.resetAt?new Date(quota.resetAt*1000).toLocaleString('es'):undefined}><Clock3 size={12}/>{countdown(quota.resetAt,now)}</span></div>
    {quota.resetAt&&<time className="ai-reset-date" dateTime={new Date(quota.resetAt*1000).toISOString()}>{new Date(quota.resetAt*1000).toLocaleString('es',{dateStyle:'medium',timeStyle:'short'})}</time>}
  </div>;
}
export default function AIUsagePanel(){
  const [data,setData]=useState<Snapshot>(empty),[loading,setLoading]=useState(native),[busy,setBusy]=useState(''),[error,setError]=useState(''),[notice,setNotice]=useState(''),[now,setNow]=useState(()=>Math.floor(Date.now()/1000));
  const mounted=useRef(true),gate=useRef(false);
  async function refresh(remote:boolean){
    if(!native||gate.current)return;gate.current=true;if(remote)setBusy('refresh');
    try{const next=await invoke<Snapshot>('ai_usage_snapshot',{refresh:remote});if(mounted.current){setData(next);setError('');}}
    catch(reason){if(mounted.current)setError(String(reason));}
    finally{gate.current=false;if(mounted.current){setLoading(false);setBusy('');}}
  }
  useEffect(()=>{
    mounted.current=true;
    if(native){void refresh(false).then(()=>{if(mounted.current)void refresh(true);});}
    let tick=0;
    const timer=setInterval(()=>{setNow(Math.floor(Date.now()/1000));if(!document.hidden&&++tick%2===0)void refresh(tick%20===0);},15000);
    const focus=()=>{if(!document.hidden)void refresh(false);};document.addEventListener('visibilitychange',focus);window.addEventListener('focus',focus);
    return()=>{mounted.current=false;clearInterval(timer);document.removeEventListener('visibilitychange',focus);window.removeEventListener('focus',focus);};
  },[]);
  async function connect(){
    if(gate.current)return;gate.current=true;setBusy('claude');setError('');
    try{const next=await invoke<Snapshot>('ai_usage_connect_claude',{connect:!data.claudeConnection.connected});if(mounted.current){setData(next);setNotice(next.claudeConnection.connected?'Claude Code conectado. Abrí una sesión de CLI en Windows y enviá un mensaje para recibir las cuotas.':'Conexión retirada. Se restauró la barra anterior de Claude Code.');}}
    catch(reason){if(mounted.current)setError(String(reason));}
    finally{gate.current=false;if(mounted.current)setBusy('');}
  }
  return <section className="ai-usage" aria-label="Uso de las suscripciones de IA" aria-busy={loading}>
    <div className="ai-toolbar"><p>Cuánto te queda en cada cuenta.</p><Button variant="secondary" disabled={!native||!!busy||loading} onClick={()=>void refresh(true)}><RefreshCw size={14} className={busy==='refresh'?'ai-refreshing':''}/>{busy==='refresh'?'Consultando…':'Actualizar cuotas'}</Button></div>
    {!native&&<p className="ai-notice">Vista previa sin conexión. Los datos de tus cuentas aparecerán en la aplicación de Windows.</p>}
    {error&&<p className="ai-notice" role="alert">{error}</p>}{notice&&<p className="ai-notice" role="status">{notice}</p>}
    <div className="ai-providers">{data.providers.map(provider=>{
      const stale=provider.observedAt!==null&&now-provider.observedAt>300;
      const previous=stale||provider.status==='error'||!!error;
      const expired=provider.windows.length>0&&provider.windows.every(w=>w.resetAt!==null&&w.resetAt<=now);
      const state=loading?'Buscando datos':provider.status==='error'?'No se pudo actualizar':expired?'Reinicio por confirmar':stale?'Datos anteriores':provider.status==='ready'?'Datos disponibles':provider.status==='waiting'?'Esperando datos':'Sin conectar';
      return <article className="ai-provider" key={provider.id} aria-label={provider.name}>
        <header><div><span className="ai-provider-kind">{provider.id==='codex'?'Cuenta de ChatGPT en Codex':'Cuenta de Claude Code'}</span><h2>{provider.name}</h2>{provider.plan&&<span className="ai-plan">Plan {provider.plan}</span>}</div><span className="ai-status" data-warning={previous||expired||provider.status==='error'}>{state}</span></header>
        {provider.windows.length>0?<div className="ai-quota-list">{provider.windows.map((quota,index)=><QuotaRow key={`${quota.label}-${index}`} quota={quota} now={now} previous={previous}/>)}</div>:<div className="ai-empty"><strong>{loading?'Consultando cuotas…':provider.status==='waiting'?'La conexión está lista. Faltan las cuotas.':'Tus cuotas aparecerán acá.'}</strong><p>{provider.message}</p></div>}
        {provider.windows.length>0&&provider.message&&<p className="ai-provider-note">{provider.message}</p>}
        {provider.credits.length>0?<dl className="ai-credits">{provider.credits.map((credit,index)=><div key={index}><dt>{credit.label}</dt><dd>{credit.unlimited?'Sin límite informado':credit.balance??'No informado'}</dd></div>)}</dl>:<p className="ai-credit-note">El servicio no informó un saldo de créditos.</p>}
        <footer><span>{provider.observedAt?`Recibido ${new Date(provider.observedAt*1000).toLocaleString('es',{dateStyle:'short',timeStyle:'short'})}`:'Todavía sin actualización'}{previous&&' · consultá el dato oficial antes de continuar'}</span><button disabled={!native} onClick={()=>void invoke('ai_usage_open_provider',{provider:provider.id}).catch(reason=>setError(String(reason)))}>Ver uso oficial<ArrowUpRight size={13}/></button></footer>
      </article>;
    })}</div>
    <section className="ai-connection" aria-label="Conexión de Claude Code"><div><h3><Link2 size={15}/>Claude Code</h3><p>La conexión configura su barra de estado con las cuotas. Guarda una copia y restaura la barra anterior al desconectar. Solo recoge porcentajes y reinicios.</p></div><Button variant="secondary" disabled={!native||!!busy||loading} onClick={()=>void connect()}>{busy==='claude'?'Guardando…':data.claudeConnection.connected?'Desconectar Claude Code':'Conectar Claude Code'}</Button></section>
    <details className="ai-help"><summary>Cómo se actualizan estos datos</summary><p>OpenAI se consulta mediante la sesión de ChatGPT que ya usa Codex. Las cuotas disponibles pueden variar según el plan; esta vista muestra las que devuelve el proveedor.</p><p>Claude envía sus cuotas desde la barra de estado de Claude Code CLI en Windows después de responder. El consumo de VS Code y de la app comparte la cuota de esa misma cuenta, pero esas interfaces pueden no ejecutar la barra. «Actualizar cuotas» vuelve a leer el último dato recibido de Claude; no obliga a sus servidores a renovarlo.</p><p>Si usás Claude solo en WSL, una configuración personalizada o una barra impuesta por tu organización, esa instalación puede no ejecutar este conector. Usá una sesión de CLI de Windows con la misma cuenta o consultá «Ver uso oficial». Los datos vencidos se muestran por confirmar.</p>{data.claudeConnection.settingsPath&&<p className="ai-settings-path">Configuración conectada: <code>{data.claudeConnection.settingsPath}</code></p>}<p>Los porcentajes pertenecen a tu cuenta, no a cada chat. Los créditos no equivalen al porcentaje del plan ni se convierten a dinero salvo que el proveedor informe USD. Whispera no realiza consultas de IA para medir el uso.</p></details>
  </section>;
}
