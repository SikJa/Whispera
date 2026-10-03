import { useState, type CSSProperties } from 'react';
import { PanelLeft, PanelTop, PanelRight } from 'lucide-react';
import { useStore } from '../store/appStore';
import { useTranslation } from '../i18n';
import '../styles/edge-placement.css';

const edges = ['left','top','right'] as const;
const icons = [PanelLeft,PanelTop,PanelRight];
export default function EdgePlacement() {
  const {t}=useTranslation();
  const selected=useStore(s=>s.edgeTransition?.active?s.edgeTransition.to:s.settings.stickPosition||'left');
  const busy=useStore(s=>Boolean(s.edgeTransition?.active));
  const change=useStore(s=>s.startEdgeTransition);
  const [error,setError]=useState('');
  return <div>
    <div className="wh-edge-tabs" role="group" aria-label={t('position.edgePlacementTitle')} style={{'--edge-index':edges.indexOf(selected)} as CSSProperties}>
      <span className="wh-edge-pill" aria-hidden="true"/>
      {edges.map((edge,index)=>{const Icon=icons[index];return <button type="button" key={edge} aria-label={t(`position.${edge}Edge`)} title={t(`position.${edge}Edge`)} aria-pressed={selected===edge} disabled={busy} onClick={()=>{setError('');void change(edge).catch(e=>setError(String(e)));}}><Icon size={17}/><span>{t(`position.${edge}`)}</span></button>;})}
    </div>
    {error&&<p role="alert" className="setting-desc">{error}</p>}
  </div>;
}
