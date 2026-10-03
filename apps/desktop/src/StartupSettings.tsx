import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { native } from './client';
import { SettingsSwitch } from './ResourceControls';

export default function StartupSettings() {
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(true);
  const [loaded, setLoaded] = useState(false);
  const [message, setMessage] = useState('');
  useEffect(() => {
    let alive = true;
    if (!native) { setBusy(false); return; }
    void invoke<boolean>('startup_enabled').then(value => {
      if (alive) { setEnabled(value); setLoaded(true); }
    }).catch(error => { if (alive) setMessage(String(error)); })
      .finally(() => { if (alive) setBusy(false); });
    return () => { alive = false; };
  }, []);
  const change = async (value: boolean) => {
    setBusy(true); setMessage('');
    try {
      await invoke('set_startup', { enabled: value });
      const saved = await invoke<boolean>('startup_enabled');
      setEnabled(saved);
      if (saved !== value) throw Error('Windows no confirmó el cambio.');
      setMessage(saved ? 'Inicio automático activado.' : 'Inicio automático desactivado.');
    } catch (error) { setMessage(String(error)); }
    finally { setBusy(false); }
  };
  return <section>
    <div className="form-row"><label htmlFor="windows-startup">Iniciar Whispera con Windows<span>Oculta en la bandeja, lista para usar tus atajos.</span></label>
      <SettingsSwitch id="windows-startup" label="Iniciar Whispera con Windows" checked={enabled} disabled={!native || busy || !loaded} onChange={value => void change(value)} />
    </div>
    {message && <p className="notice" role="status">{message}</p>}
  </section>;
}
