import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Check, Copy, LoaderCircle, Save } from 'lucide-react';
import { Button } from '../vendor/components/ui/button';
import './resource-motion.css';
import { invoke } from '@tauri-apps/api/core';
import { native } from './client';

// Adapted from the user's Kinetics Switch Spring and Copy Button resources.
export function SettingsSwitch({ id, label, checked, disabled, onChange }: { id?: string; label: string; checked: boolean; disabled?: boolean; onChange: (checked: boolean) => void }) {
  return <button id={id} type="button" role="switch" aria-label={label} aria-checked={checked} className="kin-switch" data-on={checked} disabled={disabled} onClick={() => onChange(!checked)}><span className="kin-switch-thumb" /></button>;
}

export function CopyButton({ text }: { text: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'error'>('idle');
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  async function copy() {
    clearTimeout(timer.current);
    try { if (native) await invoke('copy_text', { text }); else await navigator.clipboard.writeText(text); setState('copied'); }
    catch { setState('error'); }
    timer.current = setTimeout(() => setState('idle'), 2200);
  }
  return <Button size="lg" onClick={copy} aria-live="polite"><span className="t-icon-swap" data-state={state === 'copied' ? 'b' : 'a'} aria-hidden="true"><Copy className="t-icon" data-icon="a" /><Check className="t-icon" data-icon="b" /></span>{state === 'copied' ? 'Copiado' : state === 'error' ? 'No se pudo copiar' : 'Copiar'}</Button>;
}

export function SaveButton({ busy, onSave }: { busy: boolean; onSave: () => Promise<boolean> }) {
  const [saved, setSaved] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  return <Button size="lg" disabled={busy} aria-label={saved ? 'Guardado' : 'Guardar'} onClick={async () => { clearTimeout(timer.current); if (await onSave()) { setSaved(true); timer.current = setTimeout(() => setSaved(false), 1800); } }}>
    {busy ? <LoaderCircle data-icon="inline-start" className="busy-spin" /> : <span className="t-icon-swap" data-state={saved ? 'b' : 'a'} aria-hidden="true"><Save className="t-icon" data-icon="a" /><Check className="t-icon" data-icon="b" /></span>}<span>{saved ? 'Guardado' : 'Guardar'}</span>
  </Button>;
}

export function SectionReveal({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  useEffect(() => { const frame = requestAnimationFrame(() => setOpen(true)); return () => cancelAnimationFrame(frame); }, []);
  return <div className="settings-section t-panel-slide" data-open={open}>{children}</div>;
}
