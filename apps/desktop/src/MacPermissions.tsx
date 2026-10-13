import {useCallback, useEffect, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {Check, ExternalLink, RefreshCw, ShieldCheck} from 'lucide-react';
import {native} from './client';
import {isMac} from './platform';

type Permission = 'microphone' | 'screen' | 'accessibility';
type Permissions = Record<Permission, 'granted' | 'required' | 'denied' | 'unknown'>;
const rows = [
  {id: 'microphone', es: 'Micrófono', en: 'Microphone', detailEs: 'Dictado y voz en videos.', detailEn: 'Dictation and voice in videos.'},
  {id: 'screen', es: 'Captura de pantalla', en: 'Screen recording', detailEs: 'Capturas, videos y audio del sistema.', detailEn: 'Screenshots, videos and system audio.'},
  {id: 'accessibility', es: 'Accesibilidad', en: 'Accessibility', detailEs: 'Volver al campo original y pegar el dictado.', detailEn: 'Restore the original text field and paste dictation.'},
] as const;

export default function MacPermissions({lang = 'es'}: {lang?: 'es' | 'en'}) {
  const [status, setStatus] = useState<Permissions>();
  const [busy, setBusy] = useState<Permission>();
  const [error, setError] = useState('');
  const t = (es: string, en: string) => lang === 'es' ? es : en;
  const refresh = useCallback(async () => {
    try { setStatus(await invoke<Permissions>('platform_permissions')); setError(''); }
    catch (reason) { setError(String(reason)); }
  }, []);
  useEffect(() => {
    if (!native || !isMac) return;
    let active = true;
    void invoke<Permissions>('platform_permissions').then(value => {if (active) setStatus(value);})
      .catch(reason => {if (active) setError(String(reason));});
    const focus = () => {if (active) void refresh();};
    window.addEventListener('focus', focus);
    return () => {active = false; window.removeEventListener('focus', focus);};
  }, [refresh]);
  if (!isMac || !native) return null;
  async function request(kind: Permission, settings: boolean) {
    setBusy(kind); setError('');
    try { setStatus(await invoke<Permissions>('platform_request_permission', {kind, settings})); }
    catch (reason) { setError(String(reason)); }
    finally {setBusy(undefined);}
  }
  return <section aria-label={t('Permisos de macOS', 'macOS permissions')}>
    <h3>{t('Permisos de macOS', 'macOS permissions')}</h3>
    {rows.map(row => <div className="form-row" key={row.id}>
      <label>{row[lang]}<span>{lang === 'es' ? row.detailEs : row.detailEn}</span></label>
      {status?.[row.id] === 'granted' ? <span role="status"><Check size={16}/>{t('Permitido', 'Allowed')}</span> :
        <button disabled={Boolean(busy) || !status} onClick={() => void request(row.id, status?.[row.id] === 'denied')}>
          <ShieldCheck size={16}/>{busy === row.id ? t('Esperando…', 'Waiting…') : t('Permitir', 'Allow')}
        </button>}
      <button disabled={Boolean(busy)} title={t('Abrir ajustes del sistema', 'Open System Settings')}
        aria-label={`${t('Abrir ajustes del sistema', 'Open System Settings')}: ${row[lang]}`} onClick={() => void request(row.id, true)}><ExternalLink size={16}/></button>
    </div>)}
    <button disabled={Boolean(busy)} onClick={() => void refresh()}><RefreshCw size={16}/>{t('Comprobar permisos', 'Check permissions')}</button>
    {error && <p role="alert">{error}</p>}
  </section>;
}
