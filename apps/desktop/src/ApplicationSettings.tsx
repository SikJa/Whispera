import {useEffect, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {native} from './client';
import {SettingsSwitch} from './ResourceControls';

export default function ApplicationSettings() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const [reload, setReload] = useState(0);
  useEffect(() => {
    if (!native) return;
    let alive = true;
    void invoke<boolean>('startup_enabled').then(value => {
      if (alive) { setEnabled(value); setError(''); }
    }).catch(reason => { if (alive) setError(`No se pudo consultar el inicio automático. ${String(reason)}`); });
    return () => { alive = false; };
  }, [reload]);
  async function change(value: boolean) {
    if (busy || enabled === null) return;
    setBusy(true); setError(''); setSaved(false);
    try { setEnabled(await invoke<boolean>('set_startup', {enabled: value})); setSaved(true); }
    catch (reason) {
      setError(`No se pudo guardar el inicio automático. ${String(reason)}`);
      // A write may have succeeded before verification failed: read Windows again.
      try { setEnabled(await invoke<boolean>('startup_enabled')); } catch { setEnabled(null); }
    } finally { setBusy(false); }
  }
  return <>
    <h3>Inicio de la aplicación</h3>
    <div className="form-row">
      <label htmlFor="app-startup">Iniciar Whispera al encender la computadora
        <span>Se abre oculto en la bandeja, listo para usar tus atajos.</span>
      </label>
      <SettingsSwitch id="app-startup" label="Iniciar Whispera al encender la computadora" checked={enabled === true}
        disabled={!native || enabled === null || busy} onChange={value => void change(value)}/>
    </div>
    <p role="status" aria-live="polite">{!native ? 'Esta opción está disponible en la aplicación instalada.' : busy ? 'Guardando…' : saved ? 'Guardado automáticamente.' : enabled === null && !error ? 'Consultando Windows…' : ''}</p>
    {error && <div role="alert"><p>{error}</p><button disabled={busy} onClick={() => {setError(''); setReload(value => value + 1);}}>Volver a consultar</button></div>}
  </>;
}
