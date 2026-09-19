import { useEffect, useState } from 'react';
import { ArrowLeft, ArrowRight, Check, KeyRound, Mic, ShieldCheck } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import * as api from './client';
import './onboarding.css';

type Info = { complete: boolean; startup: boolean; microphone: string | null };
export function SetupGate({ children }: { children: React.ReactNode }) {
  const [needed, setNeeded] = useState<boolean>();
  const [error, setError] = useState('');
  const check = () => {
    setError('');
    if (!api.native) { setNeeded(new URLSearchParams(location.search).has('setup')); return; }
    invoke<Info>('setup_info').then(s => setNeeded(!s.complete)).catch(() => setError('Cannot load setup / No se pudo cargar la configuracion'));
  };
  useEffect(check, []);
  if (error) return <main className="setup-shell" role="alert">{error}<button onClick={check}>Retry / Reintentar</button></main>;
  if (needed === undefined) return <main className="setup-shell" role="status">Whispera...</main>;
  return needed ? <Onboarding onDone={() => setNeeded(false)} /> : children;
}

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [lang, setLang] = useState<'es' | 'en'>(navigator.language.startsWith('es') ? 'es' : 'en');
  const t = (es: string, en: string) => lang === 'es' ? es : en;
  const [step, setStep] = useState(0);
  const [key, setKey] = useState('');
  const [configured, setConfigured] = useState(false);
  const [settings, setSettings] = useState<api.Settings>();
  const [info, setInfo] = useState<Info>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const load = async () => {
    const data = await api.snapshot(); setSettings(data.settings); setConfigured(data.keyConfigured);
    if (api.native) setInfo(await invoke<Info>('setup_info'));
    else setInfo({ complete: false, startup: false, microphone: null });
  };
  useEffect(() => { void load().catch(e => setError(String(e))); }, []);
  async function run(action: () => Promise<void>) {
    setBusy(true); setError('');
    try { await action(); } catch (e) { setError(String(e)); } finally { setBusy(false); }
  }
  return <main className="setup-shell" lang={lang}>
    <header><img src="/cristal/128x128.png" alt="" width="48" height="48" /><strong>Whispera</strong><select aria-label="Language / Idioma" value={lang} onChange={e => setLang(e.target.value as 'es' | 'en')}><option value="es">Español</option><option value="en">English</option></select></header>
    <nav aria-label={t('Progreso', 'Progress')}>{[t('Privacidad', 'Privacy'), 'Groq', t('Preferencias', 'Preferences'), t('Listo', 'Ready')].map((label, i) => <span key={i} aria-current={step === i ? 'step' : undefined}>{i + 1}. {label}</span>)}</nav>
    <section key={step} className="setup-step">
      {step === 0 && <><ShieldCheck size={28} /><h1>{t('Tu voz, tu configuración', 'Your voice, your setup')}</h1><p>{t('Whispera envía el audio y el vocabulario de tu diccionario a Groq para transcribir. Usás tu propia cuenta; sus límites y condiciones aplican.', 'Whispera sends audio and your dictionary vocabulary to Groq for transcription. You use your own account; its limits and terms apply.')}</p><p>{t('Las grabaciones y el historial se guardan en esta PC para recuperación. No se borran automáticamente. La clave se guarda en el almacén de credenciales de Windows, no en el repositorio.', 'Recordings and history stay on this PC for recovery. They are not automatically deleted. Your key is stored in Windows Credential Manager, not in the repository.')}</p><p>{t('Sin cuenta de Whispera ni modelos locales. Podés salir desde el icono de bandeja.', 'No Whispera account or local models. Quit from the tray icon.')}</p></>}
      {step === 1 && <><KeyRound size={28} /><h1>{t('Conectá tu cuenta de Groq', 'Connect your Groq account')}</h1><p>{t('Creá una clave en console.groq.com/keys. La comprobación consulta los modelos disponibles; no envía audio.', 'Create a key at console.groq.com/keys. Validation checks available models; it does not send audio.')}</p><label htmlFor="setup-key">Groq API key</label><input id="setup-key" type="password" autoComplete="off" spellCheck={false} value={key} onChange={e => setKey(e.target.value)} placeholder="gsk_..." /><button disabled={busy || !key.trim() || !api.native} onClick={() => void run(async () => { await invoke('validate_key', { key }); setKey(''); setConfigured(true); })}>{configured ? t('Actualizar clave', 'Update key') : t('Validar y guardar', 'Validate and save')}</button>{configured && <p role="status"><Check size={16} /> {t('Clave guardada en Windows', 'Key saved in Windows')}</p>}{!api.native && <p>{t('Vista previa: las claves solo se introducen en la app instalada.', 'Preview: enter keys only in the installed app.')}</p>}</>}
      {step === 2 && settings && <><Mic size={28} /><h1>{t('Prepará tu primer dictado', 'Prepare your first dictation')}</h1><p>{t('Micrófono predeterminado de Windows:', 'Windows default microphone:')} <strong>{info?.microphone ?? t('No detectado', 'Not detected')}</strong></p><p>{t('Esto detecta el dispositivo, no verifica la señal. Revisá el permiso de micrófono de Windows y probá una frase breve al terminar.', 'This detects the device, not the signal. Check Windows microphone permissions and try a short phrase after setup.')}</p><button disabled={busy} onClick={() => void run(load)}>{t('Actualizar dispositivos', 'Refresh devices')}</button><label htmlFor="setup-hotkey">{t('Atajo global', 'Global shortcut')}</label><input id="setup-hotkey" value={settings.hotkey} onChange={e => setSettings({ ...settings, hotkey: e.target.value })} /><label htmlFor="setup-language">{t('Idioma del audio', 'Audio language')}</label><select id="setup-language" value={settings.language} onChange={e => setSettings({ ...settings, language: e.target.value })}><option value="es">Español</option><option value="en">English</option><option value="pt">Português</option><option value="auto">Auto</option></select><label className="setup-check"><input type="checkbox" checked={settings.autoPaste} onChange={e => setSettings({ ...settings, autoPaste: e.target.checked })} />{t('Pegar al terminar en la ventana original', 'Paste into the original window when finished')}</label><label className="setup-check"><input type="checkbox" checked={info?.startup ?? false} onChange={e => setInfo({ complete: false, microphone: info?.microphone ?? null, startup: e.target.checked })} />{t('Iniciar con Windows, oculto en bandeja', 'Start with Windows, hidden in the tray')}</label></>}
      {step === 3 && <><Check size={28} /><h1>{t('Listo para dictar', 'Ready to dictate')}</h1><p>{t('Hacé clic en un campo de texto y pulsá el atajo. Hablá y volvé a pulsarlo para transcribir.', 'Click a text field and press your shortcut. Speak, then press it again to transcribe.')}</p><kbd>{settings?.hotkey}</kbd><p>{t('La carpeta aparece mientras grabás. Pausá o cancelá desde sus controles. Configuración queda disponible en la bandeja.', 'The folder appears while recording. Pause or cancel using its controls. Settings remain available in the tray.')}</p><p>{t('El pegado intenta recuperar la ventana original, no una pestaña específica del navegador. El texto también queda disponible en Historial.', 'Pasting tries to restore the original window, not a specific browser tab. Text is also available in History.')}</p></>}
    </section>
    {error && <p role="alert" className="setup-error">{error}</p>}
    <footer><button disabled={step === 0 || busy} onClick={() => setStep(step - 1)}><ArrowLeft size={16} />{t('Atrás', 'Back')}</button><button disabled={busy || (step === 1 && !configured && api.native) || (step === 2 && !settings)} onClick={() => void run(async () => {
      if (step === 2 && settings && api.native) { await api.saveSettings(settings); await invoke('set_startup', { enabled: info?.startup ?? false }); }
      if (step < 3) setStep(step + 1);
      else { if (api.native) { await invoke('complete_setup'); await getCurrentWindow().hide(); } onDone(); }
    })}>{busy ? t('Guardando...', 'Saving...') : step === 3 ? t('Ir a la bandeja', 'Continue in tray') : t('Continuar', 'Continue')}<ArrowRight size={16} /></button></footer>
  </main>;
}
